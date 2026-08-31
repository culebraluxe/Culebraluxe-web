import { randomUUID } from 'node:crypto'

import { sql } from './client'
import { createPersonWithIdentities, findIdentityMatches } from './person-identities'
import { recordReconcileDecision } from './relationship-evidence'
import { isHumanName } from '../lib/relationship-intel/names'
import { REL_INTEL_RULE_VERSION } from '../lib/relationship-intel/reconcile'
import type { NormalizedIdentityHint } from '../lib/crm-intake-types'

export type MasteringIdentity = {
  kind: 'email' | 'phone'
  value: string
  sourceSystem: string
}

export type CurrentSourcePerson = {
  source: string
  sourceAccount: string
  sourceIdentityKey: string
  displayName: string
  organization: string | null
  hasPersonName: boolean
  identities: MasteringIdentity[]
}

export type MasteringOutcome =
  | 'linked_existing_source'
  | 'linked_existing_identity'
  | 'created_person'
  | 'enriched_existing'
  | 'ambiguous'
  | 'deferred'

export type MasteringResult = {
  current: number
  linkedExistingSource: number
  linkedExistingIdentity: number
  created: number
  enriched: number
  ambiguous: number
  deferred: number
  identitiesAdded: number
}

function normalizeIdentity(kind: 'email' | 'phone', value: string): string {
  if (kind === 'email') return value.trim().toLowerCase()
  return value.trim()
}

function identityKey(identity: MasteringIdentity): string {
  return `${identity.kind}:${identity.kind === 'email' ? identity.value.trim().toLowerCase() : identity.value.replace(/[^0-9]/g, '')}`
}

function dedupeIdentities(identities: MasteringIdentity[]): MasteringIdentity[] {
  const byKey = new Map<string, MasteringIdentity>()
  for (const identity of identities) {
    const value = normalizeIdentity(identity.kind, identity.value)
    if (!value) continue
    const normalized = { ...identity, value }
    const key = identityKey(normalized)
    if (!key.endsWith(':') && !byKey.has(key)) byKey.set(key, normalized)
  }
  return [...byKey.values()].sort((a, b) => identityKey(a).localeCompare(identityKey(b)))
}

async function getSourceLink(record: CurrentSourcePerson): Promise<string | null> {
  const rows = (await sql`
    select canonical_person_id
    from integration_source_person_link
    where source = ${record.source}
      and source_account = ${record.sourceAccount}
      and source_identity_key = ${record.sourceIdentityKey}
    limit 1
  `) as { canonical_person_id: string }[]
  return rows[0]?.canonical_person_id ?? null
}

async function persistSourceLink(
  record: CurrentSourcePerson,
  personId: string,
  method: string,
  reason: string,
): Promise<string> {
  await sql`
    insert into integration_source_person_link (
      source, source_account, source_identity_key,
      canonical_person_id, link_method, link_reason
    ) values (
      ${record.source}, ${record.sourceAccount}, ${record.sourceIdentityKey},
      ${personId}, ${method}, ${reason}
    )
    on conflict (source, source_account, source_identity_key) do nothing
  `

  const linked = await getSourceLink(record)
  if (!linked) throw new Error(`source-person link was not persisted for ${record.source}:${record.sourceIdentityKey}`)
  if (linked !== personId) {
    throw new Error(
      `source-person ownership conflict for ${record.source}:${record.sourceIdentityKey}; existing=${linked} attempted=${personId}`,
    )
  }
  return linked
}

async function attachSafeIdentities(
  personId: string,
  identities: MasteringIdentity[],
): Promise<{ added: number; conflict: boolean }> {
  let added = 0
  for (const identity of dedupeIdentities(identities)) {
    const matches = await findIdentityMatches({
      kind: identity.kind,
      normalizedValue: identity.value,
    } as NormalizedIdentityHint)
    const owners = new Set(matches.map((m) => m.personId))

    if (owners.size > 1) return { added, conflict: true }
    if (owners.size === 1) {
      if ([...owners][0] !== personId) return { added, conflict: true }
      continue
    }

    const inserted = await sql`
      insert into person_identity (
        person_id, identity_type, identity_value, source_system, is_primary
      ) values (
        ${personId}, ${identity.kind}, ${identity.value}, ${identity.sourceSystem}, false
      )
      on conflict (identity_type, identity_value) do nothing
      returning id
    `
    if (inserted.length > 0) added += 1
  }
  return { added, conflict: false }
}

async function writeEvidenceOutcome(
  record: CurrentSourcePerson,
  outcome: MasteringOutcome,
  personId: string | null,
  reason: string,
): Promise<void> {
  const rows = (await sql`
    select id
    from integration_relationship_evidence
    where source = ${record.source}
      and source_account = ${record.sourceAccount}
      and source_identity_key = ${record.sourceIdentityKey}
    limit 1
  `) as { id: string }[]
  const evidenceId = rows[0]?.id
  if (!evidenceId) return

  if (outcome === 'ambiguous') {
    await recordReconcileDecision(evidenceId, {
      reviewState: 'ambiguous',
      matchMethod: 'exact_email',
      matchConfidence: 'ambiguous',
      canonicalPersonId: null,
      reason,
      ruleVersion: REL_INTEL_RULE_VERSION,
    })
    return
  }
  if (outcome === 'deferred') {
    await recordReconcileDecision(evidenceId, {
      reviewState: 'deferred',
      matchMethod: 'unmatched',
      matchConfidence: 'none',
      canonicalPersonId: null,
      reason,
      ruleVersion: REL_INTEL_RULE_VERSION,
    })
    return
  }

  await recordReconcileDecision(evidenceId, {
    reviewState: 'exact_linked',
    matchMethod: outcome === 'linked_existing_source' ? 'source_link' : 'exact_email',
    matchConfidence: 'exact',
    canonicalPersonId: personId,
    reason,
    ruleVersion: REL_INTEL_RULE_VERSION,
  })
}

export async function loadCurrentSourcePeople(source: string): Promise<CurrentSourcePerson[]> {
  const rows = (await sql`
    select
      lp.id,
      lp.source,
      lp.source_account,
      lp.source_contact_id,
      lp.display_name,
      lp.organization,
      lp.given_name,
      lp.family_name,
      li.identity_type,
      li.normalized_value,
      li.identity_value
    from l_person lp
    left join l_person_identity li on li.l_person_id = lp.id
    where lp.source = ${source}
    order by lp.id, li.ordinal, li.id
  `) as {
    id: string
    source: string
    source_account: string
    source_contact_id: string
    display_name: string | null
    organization: string | null
    given_name: string | null
    family_name: string | null
    identity_type: string | null
    normalized_value: string | null
    identity_value: string | null
  }[]

  const byId = new Map<string, CurrentSourcePerson>()
  for (const row of rows) {
    let current = byId.get(row.id)
    if (!current) {
      current = {
        source: row.source,
        sourceAccount: row.source_account,
        sourceIdentityKey: row.source_contact_id,
        displayName: row.display_name?.trim() || row.source_contact_id,
        organization: row.organization,
        hasPersonName: Boolean(row.given_name?.trim() || row.family_name?.trim()),
        identities: [],
      }
      byId.set(row.id, current)
    }
    if ((row.identity_type === 'email' || row.identity_type === 'phone') && row.identity_value) {
      current.identities.push({
        kind: row.identity_type,
        value: row.normalized_value || row.identity_value,
        sourceSystem: source,
      })
    }
  }
  return [...byId.values()]
}

export async function masterCurrentSourcePeople(source: string): Promise<MasteringResult> {
  const records = await loadCurrentSourcePeople(source)
  const result: MasteringResult = {
    current: records.length,
    linkedExistingSource: 0,
    linkedExistingIdentity: 0,
    created: 0,
    enriched: 0,
    ambiguous: 0,
    deferred: 0,
    identitiesAdded: 0,
  }

  for (const record of records) {
    const identities = dedupeIdentities(record.identities)
    const orgOnly = !record.hasPersonName && Boolean(record.organization?.trim())
    if (orgOnly || identities.length === 0) {
      result.deferred += 1
      await writeEvidenceOutcome(record, 'deferred', null, orgOnly ? 'organization_or_service' : 'insufficient_identity_evidence')
      continue
    }

    const existingLink = await getSourceLink(record)
    if (existingLink) {
      const enrichment = await attachSafeIdentities(existingLink, identities)
      if (enrichment.conflict) {
        result.ambiguous += 1
        await writeEvidenceOutcome(record, 'ambiguous', existingLink, 'established_source_link_identity_conflict')
        continue
      }
      result.linkedExistingSource += 1
      result.identitiesAdded += enrichment.added
      if (enrichment.added > 0) result.enriched += 1
      await writeEvidenceOutcome(record, 'linked_existing_source', existingLink, enrichment.added > 0 ? 'source_link_enriched' : 'source_link_reused')
      continue
    }

    const distinctOwners = new Set<string>()
    let ambiguous = false
    for (const identity of identities) {
      const matches = await findIdentityMatches({
        kind: identity.kind,
        normalizedValue: identity.value,
      } as NormalizedIdentityHint)
      const owners = new Set(matches.map((m) => m.personId))
      if (owners.size > 1) ambiguous = true
      for (const owner of owners) distinctOwners.add(owner)
    }

    if (ambiguous || distinctOwners.size > 1) {
      result.ambiguous += 1
      await writeEvidenceOutcome(record, 'ambiguous', null, ambiguous ? 'identity_matches_multiple_people' : 'cross_identity_conflict')
      continue
    }

    if (distinctOwners.size === 1) {
      const personId = [...distinctOwners][0]
      const enrichment = await attachSafeIdentities(personId, identities)
      if (enrichment.conflict) {
        result.ambiguous += 1
        await writeEvidenceOutcome(record, 'ambiguous', null, 'identity_ownership_changed_during_mastering')
        continue
      }
      await persistSourceLink(record, personId, 'exact_identity', 'unique_normalized_identity_owner')
      result.linkedExistingIdentity += 1
      result.identitiesAdded += enrichment.added
      if (enrichment.added > 0) result.enriched += 1
      await writeEvidenceOutcome(record, 'linked_existing_identity', personId, enrichment.added > 0 ? 'identity_linked_enriched' : 'identity_linked_existing')
      continue
    }

    const personId = randomUUID()
    const primary = identities[0]
    await createPersonWithIdentities({
      personId,
      displayName: record.displayName,
      role: 'unclassified',
      identities: identities.map((identity) => ({
        kind: identity.kind,
        normalizedValue: identity.value,
        sourceSystem: identity.sourceSystem,
        isPrimary: identityKey(identity) === identityKey(primary),
      })),
    })
    await sql`
      update person
      set display_name_source = ${isHumanName(record.displayName) ? 'source_evidence' : 'identity_fallback'}
      where id = ${personId}
    `
    await persistSourceLink(record, personId, 'new_identity', 'no_existing_canonical_identity_owner')
    result.created += 1
    result.identitiesAdded += identities.length
    await writeEvidenceOutcome(record, 'created_person', personId, 'mastered_new_person')
  }

  return result
}
