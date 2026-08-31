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

export type MasteringProgress = MasteringResult & {
  processed: number
  elapsedMs: number
}

export type MasteringOptions = {
  progressEvery?: number
  onProgress?: (progress: MasteringProgress) => void
}

type EvidenceState = {
  id: string
  canonicalPersonId: string | null
  reviewState: string
  matchMethod: string | null
  matchConfidence: string | null
  matchReason: string | null
  ruleVersion: string | null
}

type MasteringCaches = {
  sourceLinks: Map<string, string>
  identityOwners: Map<string, Set<string>>
  evidence: Map<string, EvidenceState>
}

function normalizeIdentity(kind: 'email' | 'phone', value: string): string {
  if (kind === 'email') return value.trim().toLowerCase()
  return value.trim()
}

function identityKey(identity: MasteringIdentity): string {
  return `${identity.kind}:${identity.kind === 'email' ? identity.value.trim().toLowerCase() : identity.value.replace(/[^0-9]/g, '')}`
}

function sourceKey(record: Pick<CurrentSourcePerson, 'source' | 'sourceAccount' | 'sourceIdentityKey'>): string {
  return `${record.source}\u0000${record.sourceAccount}\u0000${record.sourceIdentityKey}`
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

async function loadMasteringCaches(source: string): Promise<MasteringCaches> {
  const [sourceLinkRows, identityRows, evidenceRows] = await Promise.all([
    sql`
      select source, source_account, source_identity_key, canonical_person_id
      from integration_source_person_link
      where source = ${source}
    `,
    sql`
      select pi.person_id, pi.identity_type, pi.identity_value
      from person_identity pi
      join person p on p.id = pi.person_id
      where pi.identity_type in ('email', 'phone')
        and p.archived_at is null
    `,
    sql`
      select
        id, source, source_account, source_identity_key,
        canonical_person_id, review_state, match_method, match_confidence,
        match_reason, rule_version
      from integration_relationship_evidence
      where source = ${source}
    `,
  ])

  const sourceLinks = new Map<string, string>()
  for (const row of sourceLinkRows as {
    source: string
    source_account: string
    source_identity_key: string
    canonical_person_id: string
  }[]) {
    sourceLinks.set(
      `${row.source}\u0000${row.source_account}\u0000${row.source_identity_key}`,
      row.canonical_person_id,
    )
  }

  const identityOwners = new Map<string, Set<string>>()
  for (const row of identityRows as {
    person_id: string
    identity_type: 'email' | 'phone'
    identity_value: string
  }[]) {
    const key = identityKey({
      kind: row.identity_type,
      value: row.identity_value,
      sourceSystem: '',
    })
    let owners = identityOwners.get(key)
    if (!owners) {
      owners = new Set<string>()
      identityOwners.set(key, owners)
    }
    owners.add(row.person_id)
  }

  const evidence = new Map<string, EvidenceState>()
  for (const row of evidenceRows as {
    id: string
    source: string
    source_account: string
    source_identity_key: string
    canonical_person_id: string | null
    review_state: string
    match_method: string | null
    match_confidence: string | null
    match_reason: string | null
    rule_version: string | null
  }[]) {
    evidence.set(`${row.source}\u0000${row.source_account}\u0000${row.source_identity_key}`, {
      id: row.id,
      canonicalPersonId: row.canonical_person_id,
      reviewState: row.review_state,
      matchMethod: row.match_method,
      matchConfidence: row.match_confidence,
      matchReason: row.match_reason,
      ruleVersion: row.rule_version,
    })
  }

  return { sourceLinks, identityOwners, evidence }
}

async function getSourceLink(
  record: CurrentSourcePerson,
  caches?: MasteringCaches,
): Promise<string | null> {
  const key = sourceKey(record)
  if (caches?.sourceLinks.has(key)) return caches.sourceLinks.get(key) ?? null

  const rows = (await sql`
    select canonical_person_id
    from integration_source_person_link
    where source = ${record.source}
      and source_account = ${record.sourceAccount}
      and source_identity_key = ${record.sourceIdentityKey}
    limit 1
  `) as { canonical_person_id: string }[]
  const personId = rows[0]?.canonical_person_id ?? null
  if (personId && caches) caches.sourceLinks.set(key, personId)
  return personId
}

async function persistSourceLink(
  record: CurrentSourcePerson,
  personId: string,
  method: string,
  reason: string,
  caches?: MasteringCaches,
): Promise<string> {
  const key = sourceKey(record)
  const cached = caches?.sourceLinks.get(key)
  if (cached) {
    if (cached !== personId) {
      throw new Error(
        `source-person ownership conflict for ${record.source}:${record.sourceIdentityKey}; existing=${cached} attempted=${personId}`,
      )
    }
    return cached
  }

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
  caches?.sourceLinks.set(key, linked)
  return linked
}

async function ownersForIdentity(
  identity: MasteringIdentity,
  caches?: MasteringCaches,
): Promise<Set<string>> {
  const key = identityKey(identity)
  const cached = caches?.identityOwners.get(key)
  if (cached) return new Set(cached)

  const matches = await findIdentityMatches({
    kind: identity.kind,
    normalizedValue: identity.value,
  } as NormalizedIdentityHint)
  const owners = new Set(matches.map((m) => m.personId))
  if (caches) caches.identityOwners.set(key, new Set(owners))
  return owners
}

async function attachSafeIdentities(
  personId: string,
  identities: MasteringIdentity[],
  caches?: MasteringCaches,
): Promise<{ added: number; conflict: boolean }> {
  const normalized = dedupeIdentities(identities)

  // Preflight the entire set before writing anything. A conflict must never
  // partially enrich the Person before the later identity is discovered.
  for (const identity of normalized) {
    const owners = await ownersForIdentity(identity, caches)
    if (owners.size > 1) return { added: 0, conflict: true }
    if (owners.size === 1 && !owners.has(personId)) return { added: 0, conflict: true }
  }

  let added = 0
  for (const identity of normalized) {
    const key = identityKey(identity)
    const owners = caches?.identityOwners.get(key)
    if (owners?.has(personId)) continue

    const inserted = await sql`
      insert into person_identity (
        person_id, identity_type, identity_value, source_system, is_primary
      ) values (
        ${personId}, ${identity.kind}, ${identity.value}, ${identity.sourceSystem}, false
      )
      on conflict (identity_type, identity_value) do nothing
      returning id
    `

    if (inserted.length > 0) {
      added += 1
      if (caches) caches.identityOwners.set(key, new Set([personId]))
      continue
    }

    // Another writer may have claimed the identity since the cache snapshot.
    // Re-read just this identity and fail closed if ownership changed.
    const matches = await findIdentityMatches({
      kind: identity.kind,
      normalizedValue: identity.value,
    } as NormalizedIdentityHint)
    const refreshedOwners = new Set(matches.map((m) => m.personId))
    if (caches) caches.identityOwners.set(key, new Set(refreshedOwners))
    if (refreshedOwners.size !== 1 || !refreshedOwners.has(personId)) {
      return { added, conflict: true }
    }
  }
  return { added, conflict: false }
}

function evidenceDecisionFor(
  outcome: MasteringOutcome,
  personId: string | null,
  reason: string,
): {
  reviewState: 'ambiguous' | 'deferred' | 'exact_linked'
  matchMethod: string
  matchConfidence: 'ambiguous' | 'none' | 'exact'
  canonicalPersonId: string | null
  reason: string
} {
  if (outcome === 'ambiguous') {
    return {
      reviewState: 'ambiguous',
      matchMethod: 'exact_email',
      matchConfidence: 'ambiguous',
      canonicalPersonId: null,
      reason,
    }
  }
  if (outcome === 'deferred') {
    return {
      reviewState: 'deferred',
      matchMethod: 'unmatched',
      matchConfidence: 'none',
      canonicalPersonId: null,
      reason,
    }
  }
  return {
    reviewState: 'exact_linked',
    matchMethod: outcome === 'linked_existing_source' ? 'source_link' : 'exact_email',
    matchConfidence: 'exact',
    canonicalPersonId: personId,
    reason,
  }
}

async function writeEvidenceOutcome(
  record: CurrentSourcePerson,
  outcome: MasteringOutcome,
  personId: string | null,
  reason: string,
  caches?: MasteringCaches,
): Promise<void> {
  const key = sourceKey(record)
  let current = caches?.evidence.get(key)

  if (!current) {
    const rows = (await sql`
      select
        id, canonical_person_id, review_state, match_method,
        match_confidence, match_reason, rule_version
      from integration_relationship_evidence
      where source = ${record.source}
        and source_account = ${record.sourceAccount}
        and source_identity_key = ${record.sourceIdentityKey}
      limit 1
    `) as {
      id: string
      canonical_person_id: string | null
      review_state: string
      match_method: string | null
      match_confidence: string | null
      match_reason: string | null
      rule_version: string | null
    }[]
    const row = rows[0]
    if (!row) return
    current = {
      id: row.id,
      canonicalPersonId: row.canonical_person_id,
      reviewState: row.review_state,
      matchMethod: row.match_method,
      matchConfidence: row.match_confidence,
      matchReason: row.match_reason,
      ruleVersion: row.rule_version,
    }
    caches?.evidence.set(key, current)
  }

  const desired = evidenceDecisionFor(outcome, personId, reason)
  if (
    current.canonicalPersonId === desired.canonicalPersonId &&
    current.reviewState === desired.reviewState &&
    current.matchMethod === desired.matchMethod &&
    current.matchConfidence === desired.matchConfidence &&
    current.matchReason === desired.reason &&
    current.ruleVersion === REL_INTEL_RULE_VERSION
  ) {
    return
  }

  await recordReconcileDecision(current.id, {
    reviewState: desired.reviewState,
    matchMethod: desired.matchMethod,
    matchConfidence: desired.matchConfidence,
    canonicalPersonId: desired.canonicalPersonId,
    reason: desired.reason,
    ruleVersion: REL_INTEL_RULE_VERSION,
  })

  caches?.evidence.set(key, {
    id: current.id,
    canonicalPersonId: desired.canonicalPersonId,
    reviewState: desired.reviewState,
    matchMethod: desired.matchMethod,
    matchConfidence: desired.matchConfidence,
    matchReason: desired.reason,
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

export async function masterCurrentSourcePeople(
  source: string,
  options: MasteringOptions = {},
): Promise<MasteringResult> {
  const records = await loadCurrentSourcePeople(source)
  const caches = await loadMasteringCaches(source)
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
  const startedAt = Date.now()
  const progressEvery = Math.max(1, options.progressEvery ?? 100)

  const reportProgress = (processed: number) => {
    if (!options.onProgress) return
    options.onProgress({
      ...result,
      processed,
      elapsedMs: Date.now() - startedAt,
    })
  }

  for (let index = 0; index < records.length; index += 1) {
    const record = records[index]
    const identities = dedupeIdentities(record.identities)
    const orgOnly = !record.hasPersonName && Boolean(record.organization?.trim())
    if (orgOnly || identities.length === 0) {
      result.deferred += 1
      await writeEvidenceOutcome(record, 'deferred', null, orgOnly ? 'organization_or_service' : 'insufficient_identity_evidence', caches)
    } else {
      const existingLink = await getSourceLink(record, caches)
      if (existingLink) {
        const enrichment = await attachSafeIdentities(existingLink, identities, caches)
        if (enrichment.conflict) {
          result.ambiguous += 1
          await writeEvidenceOutcome(record, 'ambiguous', existingLink, 'established_source_link_identity_conflict', caches)
        } else {
          result.linkedExistingSource += 1
          result.identitiesAdded += enrichment.added
          if (enrichment.added > 0) result.enriched += 1
          await writeEvidenceOutcome(record, 'linked_existing_source', existingLink, enrichment.added > 0 ? 'source_link_enriched' : 'source_link_reused', caches)
        }
      } else {
        const distinctOwners = new Set<string>()
        let ambiguous = false
        for (const identity of identities) {
          const owners = await ownersForIdentity(identity, caches)
          if (owners.size > 1) ambiguous = true
          for (const owner of owners) distinctOwners.add(owner)
        }

        if (ambiguous || distinctOwners.size > 1) {
          result.ambiguous += 1
          await writeEvidenceOutcome(record, 'ambiguous', null, ambiguous ? 'identity_matches_multiple_people' : 'cross_identity_conflict', caches)
        } else if (distinctOwners.size === 1) {
          const personId = [...distinctOwners][0]
          const enrichment = await attachSafeIdentities(personId, identities, caches)
          if (enrichment.conflict) {
            result.ambiguous += 1
            await writeEvidenceOutcome(record, 'ambiguous', null, 'identity_ownership_changed_during_mastering', caches)
          } else {
            await persistSourceLink(record, personId, 'exact_identity', 'unique_normalized_identity_owner', caches)
            result.linkedExistingIdentity += 1
            result.identitiesAdded += enrichment.added
            if (enrichment.added > 0) result.enriched += 1
            await writeEvidenceOutcome(record, 'linked_existing_identity', personId, enrichment.added > 0 ? 'identity_linked_enriched' : 'identity_linked_existing', caches)
          }
        } else {
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
          for (const identity of identities) {
            caches.identityOwners.set(identityKey(identity), new Set([personId]))
          }
          await persistSourceLink(record, personId, 'new_identity', 'no_existing_canonical_identity_owner', caches)
          result.created += 1
          result.identitiesAdded += identities.length
          await writeEvidenceOutcome(record, 'created_person', personId, 'mastered_new_person', caches)
        }
      }
    }

    const processed = index + 1
    if (processed % progressEvery === 0 || processed === records.length) reportProgress(processed)
  }

  return result
}
