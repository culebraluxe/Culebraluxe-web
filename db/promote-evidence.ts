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
import type { NormalizedIdentityHint } from '../lib/crm-intake-types'

type IdentityItem = { value: string; normalized?: string | null; label?: string | null }

export type PromotionIdentity = { kind: 'phone' | 'email'; value: string; label: string }

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

export type PromoteResult = {
  created: number
  linkedExisting: number
  groups: number
  evidenceLinked: number
}

/**
 * Promote review-required evidence into canonical Persons (deduped by identity)
 * and link each evidence row to its canonical Person. Idempotent: re-running
 * finds the just-created identity and links existing instead of duplicating.
 */
export async function promoteReviewRequiredEvidence(): Promise<PromoteResult> {
  const raw = (await sql`
    select id, source, display_name, emails, phones
    from integration_relationship_evidence
    where review_state = 'review_required'
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
  const result: PromoteResult = { created: 0, linkedExisting: 0, groups: groups.length, evidenceLinked: 0 }

  for (const group of groups) {
    const existing = await findIdentityMatch({
      kind: group.identity.kind,
      normalizedValue: group.identity.value,
    } as NormalizedIdentityHint)

    let personId: string
    let created: boolean
    if (existing) {
      personId = existing.personId
      created = false
    } else {
      personId = randomUUID()
      const displayName = displayNameForEvidence(rows, group)
      await createPersonWithIdentities({
        personId,
        displayName,
        role: 'buyer',
        identities: [
          {
            kind: group.identity.kind,
            normalizedValue: group.identity.value,
            sourceSystem: group.sourceSystems[0] ?? null,
            isPrimary: true,
          },
        ],
      })
      // Provenance for the canonical display name (CORE: identity != display name).
      await sql`
        update person
        set display_name_source = ${isHumanName(displayName) ? 'source_evidence' : 'identity_fallback'}
        where id = ${personId}
      `
      created = true
    }

    for (const evidenceId of group.evidenceIds) {
      await recordReconcileDecision(evidenceId, {
        reviewState: 'exact_linked',
        matchMethod: group.identity.kind === 'phone' ? 'exact_phone' : 'exact_email',
        matchConfidence: 'exact',
        canonicalPersonId: personId,
        reason: created ? 'promoted_new_identity' : 'promoted_linked_existing',
        ruleVersion: REL_INTEL_RULE_VERSION,
      })
      result.evidenceLinked += 1
    }

    if (created) result.created += 1
    else result.linkedExisting += 1
  }

  // Refresh the materialized client read models after the canonical promotion
  // cycle (the defined refresh boundary).
  await refreshClientReadModels()
  return result
}

