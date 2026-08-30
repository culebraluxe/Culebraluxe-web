// ---------------------------------------------------------------------------
// REL-INTEL — evidence -> canonical Person promotion (the "transform into the
// PARENT" step). Load tables (l_* / integration_relationship_evidence) are
// staging; the primary Clients screen pages the canonical `person` parent.
//
// Promotion follows the existing reconciliation rules and never blindly
// duplicates Persons:
//   - exact match            -> already linked (skip)
//   - new valid identity     -> create a canonical Person (one per unique
//                               phone/email, deduped) and link the evidence
//   - unresolved / review    -> remains staged until resolved
//
// Only rows that reconcile to `review_required` with a usable phone/email and
// are NOT automated/bulk or organization/service are promoted. Unmatched,
// deferred, rejected, non_person, and ambiguous rows stay staged for
// stewardship. The same evidence rows are preserved (never deleted).
// ---------------------------------------------------------------------------
import { randomUUID } from 'node:crypto'

import { sql } from './client'
import { findIdentityMatch, createPersonWithIdentities } from './person-identities'
import { recordReconcileDecision } from './relationship-evidence'
import { refreshClientReadModels } from './client-read-models'
import { REL_INTEL_RULE_VERSION } from '../lib/relationship-intel/reconcile'
import { isHumanName } from '../lib/relationship-intel/names'
import type { ReviewState } from '../lib/relationship-intel/contracts'
import type { NormalizedIdentityHint } from '../lib/crm-intake-types'

type IdentityItem = { value: string; normalized?: string | null; label?: string | null }

export type PromotionIdentity = { kind: 'phone' | 'email'; value: string; label: string }

/** A normalized identity intended for persistence; at most one is primary. */
export type PromotionIdentityToAttach = PromotionIdentity & { isPrimary: boolean }

export type PromotionEvidence = {
  id: string
  displayName: string | null
  source: string
  emails: IdentityItem[]
  phones: IdentityItem[]
}

export type PromotionGroup = {
  key: string
  identity: PromotionIdentity
  sourceSystems: string[]
  evidenceIds: string[]
}

/** Pick the single primary identity for a promoted canonical Person. */
export function pickPrimaryIdentity(
  emails: IdentityItem[],
  phones: IdentityItem[],
): PromotionIdentity | null {
  const phone = phones.find((p) => typeof p?.normalized === 'string' && p.normalized)
  if (phone) return { kind: 'phone', value: phone.normalized as string, label: phone.value ?? (phone.normalized as string) }
  const email = emails.find((e) => typeof e?.normalized === 'string' && e.normalized)
  if (email) return { kind: 'email', value: email.normalized as string, label: email.value ?? (email.normalized as string) }
  return null
}

/**
 * Pure grouping: dedupe evidence rows by primary identity so one identity maps
 * to exactly one canonical Person (no blind duplication). Deterministic order.
 */
export function groupEvidenceForPromotion(rows: PromotionEvidence[]): PromotionGroup[] {
  const byKey = new Map<string, PromotionGroup>()
  for (const row of rows) {
    const ident = pickPrimaryIdentity(row.emails, row.phones)
    if (!ident) continue
    const key = `${ident.kind}:${ident.value}`
    let group = byKey.get(key)
    if (!group) {
      group = { key, identity: ident, sourceSystems: [], evidenceIds: [] }
      byKey.set(key, group)
    }
    group.evidenceIds.push(row.id)
    if (!group.sourceSystems.includes(row.source)) group.sourceSystems.push(row.source)
  }
  return [...byKey.values()].sort((a, b) => a.key.localeCompare(b.key))
}

/** Deterministic display name for a promoted canonical Person. */
export function displayNameForEvidence(rows: PromotionEvidence[], group: PromotionGroup): string {
  const named = rows
    .filter((r) => group.evidenceIds.includes(r.id))
    .map((r) => r.displayName?.trim() ?? '')
    .find((n) => n.length > 0)
  return named ?? group.identity.label
}

/**
 * Collect ALL safe normalized identities from raw evidence lists, deduped by
 * `${kind}:${value}`. No primary is assigned here (all isPrimary: false).
 */
export function normalizeEvidenceIdentities(
  emails: IdentityItem[],
  phones: IdentityItem[],
): PromotionIdentityToAttach[] {
  const seen = new Set<string>()
  const out: PromotionIdentityToAttach[] = []
  const lists: Array<['email' | 'phone', IdentityItem[]]> = [
    ['email', emails],
    ['phone', phones],
  ]
  for (const [kind, list] of lists) {
    for (const item of list) {
      const normalized =
        typeof item?.normalized === 'string' && item.normalized ? item.normalized : null
      if (!normalized) continue
      const key = `${kind}:${normalized}`
      if (seen.has(key)) continue
      seen.add(key)
      out.push({ kind, value: normalized, label: item.value ?? normalized, isPrimary: false })
    }
  }
  return out
}

/**
 * Collect the FULL safe identity set for a promotion group (all emails + phones
 * across every evidence row in the group), deduped, with exactly one primary
 * (the group's deterministic primary identity).
 */
export function collectSafeIdentities(
  rows: PromotionEvidence[],
  group: PromotionGroup,
): PromotionIdentityToAttach[] {
  const identities: PromotionIdentityToAttach[] = []
  for (const row of rows) {
    if (!group.evidenceIds.includes(row.id)) continue
    for (const identity of normalizeEvidenceIdentities(row.emails, row.phones)) {
      const key = `${identity.kind}:${identity.value}`
      const existing = identities.find((i) => `${i.kind}:${i.value}` === key)
      if (!existing) identities.push(identity)
    }
  }
  // Deterministic primary: the group's primary identity.
  return identities.map((identity) => ({
    ...identity,
    isPrimary: identity.kind === group.identity.kind && identity.value === group.identity.value,
  }))
}

export type IdentityAttachmentPlan = {
  /** Identities that are currently unused and safe to attach (non-primary). */
  attach: PromotionIdentityToAttach[]
  /** Identities already owned by the target Person (replay/no-op). */
  duplicate: number
  /** Identities owned by a DIFFERENT Person — never moved, never merged. */
  conflicts: { key: string; personId: string }[]
}

/**
 * Pure enrichment plan for one target canonical Person. Given an identity set
 * and the per-identity owner (null = unused), decide what to attach, what is a
 * duplicate, and what is a conflict. Never moves an identity and never merges.
 */
export function planIdentityAttachments(
  identities: PromotionIdentityToAttach[],
  ownership: (string | null)[],
  targetPersonId: string,
): IdentityAttachmentPlan {
  const attach: PromotionIdentityToAttach[] = []
  const conflicts: { key: string; personId: string }[] = []
  let duplicate = 0
  identities.forEach((identity, index) => {
    const owner = ownership[index] ?? null
    if (owner === targetPersonId) {
      duplicate += 1
    } else if (owner === null) {
      attach.push({ ...identity, isPrimary: false })
    } else {
      conflicts.push({ key: `${identity.kind}:${identity.value}`, personId: owner })
    }
  })
  return { attach, duplicate, conflicts }
}

export type PromoteResult = {
  created: number
  linkedExisting: number
  /** existing canonical Persons that gained one or more safe identities */
  enriched: number
  /** total safe identities attached (new Persons + enrichment) */
  identitiesAdded: number
  /** evidence rows that failed closed as ambiguous (conflict, no merge/move) */
  conflicts: number
  groups: number
  evidenceLinked: number
}

/**
 * Promote safe evidence into canonical Persons (deduped by identity) and link
 * each evidence row to its canonical Person. Idempotent: re-running finds the
 * just-created identity and links existing instead of duplicating.
 *
 * Only rows that reconcile to one of `opts.reviewStates` with a usable phone/
 * email, are NOT automated/bulk or organization/service, and are NOT yet linked
 * are promoted. Ambiguous, rejected, non_person, and deferred rows stay staged.
 * The same evidence rows are preserved (never deleted). The client read models
 * are refreshed once per promotion cycle.
 */
export async function promoteEvidence(opts: {
  source?: string
  reviewStates: ReviewState[]
}): Promise<PromoteResult> {
  const raw = (opts.source
    ? await sql`
        select id, source, display_name, emails, phones
        from integration_relationship_evidence
        where source = ${opts.source}
          and review_state = any(${opts.reviewStates})
          and canonical_person_id is null
          and (is_automated_or_bulk is not true)
          and (is_organization_or_service is not true)
          and (has_email = true or has_phone = true)
      `
    : await sql`
        select id, source, display_name, emails, phones
        from integration_relationship_evidence
        where review_state = any(${opts.reviewStates})
          and canonical_person_id is null
          and (is_automated_or_bulk is not true)
          and (is_organization_or_service is not true)
          and (has_email = true or has_phone = true)
      `) as unknown[]

  const rows: PromotionEvidence[] = raw.map((r) => {
    const row = r as {
      id: string
      source: string
      display_name: string | null
      emails: unknown
      phones: unknown
    }
    return {
      id: row.id,
      source: row.source,
      displayName: row.display_name,
      emails: Array.isArray(row.emails) ? (row.emails as IdentityItem[]) : [],
      phones: Array.isArray(row.phones) ? (row.phones as IdentityItem[]) : [],
    }
  })

  const groups = groupEvidenceForPromotion(rows)
  const result: PromoteResult = {
    created: 0,
    linkedExisting: 0,
    enriched: 0,
    identitiesAdded: 0,
    conflicts: 0,
    groups: groups.length,
    evidenceLinked: 0,
  }

  for (const group of groups) {
    // Full safe identity set (all emails + phones, deduped) so the canonical
    // Person retains EVERY safe normalized identity, not just the reconciliation key.
    const identities = collectSafeIdentities(rows, group)
    const ownership = await Promise.all(
      identities.map((identity) =>
        findIdentityMatch({
          kind: identity.kind,
          normalizedValue: identity.value,
        } as NormalizedIdentityHint),
      ),
    )

    const distinctOwners = new Set<string>()
    for (const match of ownership) if (match) distinctOwners.add(match.personId)

    // Cross-identity conflict (email->A, phone->B): fail closed, never create a
    // Person claiming another's identity and never move an identity.
    if (distinctOwners.size > 1) {
      for (const evidenceId of group.evidenceIds) {
        await recordReconcileDecision(evidenceId, {
          reviewState: 'ambiguous',
          matchMethod: group.identity.kind === 'phone' ? 'exact_phone' : 'exact_email',
          matchConfidence: 'ambiguous',
          canonicalPersonId: null,
          reason: 'promotion_cross_identity_conflict',
          ruleVersion: REL_INTEL_RULE_VERSION,
        })
        result.evidenceLinked += 1
      }
      result.conflicts += 1
      continue
    }

    // Exactly one distinct owner -> enrich that Person with safe missing
    // identities; its existing legitimate buyer/seller/both role is never touched.
    if (distinctOwners.size === 1) {
      const personId = [...distinctOwners][0]
      const plan = planIdentityAttachments(
        identities,
        ownership.map((m) => (m ? m.personId : null)),
        personId,
      )
      if (plan.conflicts.length > 0) {
        for (const evidenceId of group.evidenceIds) {
          await recordReconcileDecision(evidenceId, {
            reviewState: 'ambiguous',
            matchMethod: group.identity.kind === 'phone' ? 'exact_phone' : 'exact_email',
            matchConfidence: 'ambiguous',
            canonicalPersonId: null,
            reason: 'promotion_identity_conflict',
            ruleVersion: REL_INTEL_RULE_VERSION,
          })
          result.evidenceLinked += 1
        }
        result.conflicts += 1
        continue
      }
      const added = await attachMissingIdentities(personId, plan.attach, group.sourceSystems[0] ?? null)
      result.identitiesAdded += added
      if (added > 0) result.enriched += 1
      for (const evidenceId of group.evidenceIds) {
        await recordReconcileDecision(evidenceId, {
          reviewState: 'exact_linked',
          matchMethod: group.identity.kind === 'phone' ? 'exact_phone' : 'exact_email',
          matchConfidence: 'exact',
          canonicalPersonId: personId,
          reason: added > 0 ? 'promoted_linked_enriched' : 'promoted_linked_existing',
          ruleVersion: REL_INTEL_RULE_VERSION,
        })
        result.evidenceLinked += 1
      }
      result.linkedExisting += 1
      continue
    }

    // No owner -> create ONE canonical Person retaining ALL safe identities.
    // Passive Apple evidence proves WHO a person is, not whether they are a
    // buyer/seller, so the role is 'unclassified' (never a fabricated 'buyer').
    const personId = randomUUID()
    const displayName = displayNameForEvidence(rows, group)
    await createPersonWithIdentities({
      personId,
      displayName,
      role: 'unclassified',
      identities: identities.map((identity) => ({
        kind: identity.kind,
        normalizedValue: identity.value,
        sourceSystem: group.sourceSystems[0] ?? null,
        isPrimary: identity.isPrimary,
      })),
    })
    // Provenance for the canonical display name (CORE: identity != display name).
    await sql`
      update person
      set display_name_source = ${isHumanName(displayName) ? 'source_evidence' : 'identity_fallback'}
      where id = ${personId}
    `
    result.identitiesAdded += identities.length
    result.created += 1
    for (const evidenceId of group.evidenceIds) {
      await recordReconcileDecision(evidenceId, {
        reviewState: 'exact_linked',
        matchMethod: group.identity.kind === 'phone' ? 'exact_phone' : 'exact_email',
        matchConfidence: 'exact',
        canonicalPersonId: personId,
        reason: 'promoted_new_identity',
        ruleVersion: REL_INTEL_RULE_VERSION,
      })
      result.evidenceLinked += 1
    }
  }

  // Enrich people that are ALREADY exact-linked (e.g. earlier PROD runs) but may
  // be missing secondary Apple identities. Replay-safe and conflict-aware.
  const enrichment = await enrichExactLinkedEvidence(opts.source)
  result.enriched += enrichment.enriched
  result.identitiesAdded += enrichment.identitiesAdded
  result.conflicts += enrichment.conflicts

  // Refresh the materialized client read models after the canonical promotion
  // cycle (the defined refresh boundary).
  await refreshClientReadModels()
  return result
}

/**
 * Attach safe missing identities to an existing canonical Person. Idempotent:
 * `on conflict (identity_type, identity_value) do nothing` makes a replay a
 * no-op and never moves an identity that now belongs to another Person.
 * Returns how many identities were newly attached.
 */
async function attachMissingIdentities(
  personId: string,
  identities: PromotionIdentityToAttach[],
  sourceSystem: string | null,
): Promise<number> {
  let added = 0
  for (const identity of identities) {
    const res = await sql`
      insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
      values (${personId}, ${identity.kind}, ${identity.value}, ${sourceSystem}, ${identity.isPrimary})
      on conflict (identity_type, identity_value) do nothing
      returning id
    `
    if ((res as { id: string }[]).length > 0) added += 1
  }
  return added
}

/**
 * Enrich already exact-linked canonical Persons with any safe normalized
 * identities still missing from their evidence. A conflict (an identity owned by
 * a DIFFERENT Person) fails closed for that record: the evidence is marked
 * ambiguous, nothing is attached, nothing is moved, and no merge is invented.
 */
async function enrichExactLinkedEvidence(
  source?: string,
): Promise<{ enriched: number; identitiesAdded: number; conflicts: number }> {
  const raw = (source
    ? await sql`
        select id, source, canonical_person_id, emails, phones
        from integration_relationship_evidence
        where source = ${source}
          and review_state = 'exact_linked'
          and canonical_person_id is not null
          and (is_automated_or_bulk is not true)
          and (is_organization_or_service is not true)
      `
    : await sql`
        select id, source, canonical_person_id, emails, phones
        from integration_relationship_evidence
        where review_state = 'exact_linked'
          and canonical_person_id is not null
          and (is_automated_or_bulk is not true)
          and (is_organization_or_service is not true)
      `) as unknown[]

  let enriched = 0
  let identitiesAdded = 0
  let conflicts = 0

  for (const rawRow of raw) {
    const row = rawRow as {
      id: string
      source: string
      canonical_person_id: string
      emails: unknown
      phones: unknown
    }
    const personId = row.canonical_person_id
    const identities = normalizeEvidenceIdentities(
      Array.isArray(row.emails) ? (row.emails as IdentityItem[]) : [],
      Array.isArray(row.phones) ? (row.phones as IdentityItem[]) : [],
    )
    if (identities.length === 0) continue

    const ownership = await Promise.all(
      identities.map((identity) =>
        findIdentityMatch({
          kind: identity.kind,
          normalizedValue: identity.value,
        } as NormalizedIdentityHint),
      ),
    )
    const plan = planIdentityAttachments(
      identities,
      ownership.map((m) => (m ? m.personId : null)),
      personId,
    )

    // A conflict stays reviewable/ambiguous; attach NOTHING and move NOTHING.
    if (plan.conflicts.length > 0) {
      await recordReconcileDecision(row.id, {
        reviewState: 'ambiguous',
        matchMethod: 'exact_email',
        matchConfidence: 'ambiguous',
        canonicalPersonId: null,
        reason: 'enrichment_identity_conflict',
        ruleVersion: REL_INTEL_RULE_VERSION,
      })
      conflicts += 1
      continue
    }

    const added = await attachMissingIdentities(personId, plan.attach, row.source)
    identitiesAdded += added
    if (added > 0) enriched += 1
  }

  return { enriched, identitiesAdded, conflicts }
}

/**
 * Backward-compatible wrapper: promote the generic `review_required` evidence
 * across all sources (the original single-state promotion).
 */
export async function promoteReviewRequiredEvidence(): Promise<PromoteResult> {
  return promoteEvidence({ reviewStates: ['review_required'] })
}

