// ---------------------------------------------------------------------------
// REL-INTEL — deterministic, explainable reconciliation.
//
// Every evidence row ends in one explainable outcome. Weak fuzzy-name similarity
// is NEVER an automatic match; only exact normalized email / phone or an explicit
// source link auto-link. Multiple matches for one identity produce a reviewable
// conflict, never a silent choice. Automated/bulk and service/organization
// senders are suppressed, and canonical Persons are never silently merged.
// ---------------------------------------------------------------------------

import type {
  MatchConfidence,
  MatchMethod,
  ReconcileDecision,
  RelationshipEvidence,
  ReviewState,
} from './contracts'

export const REL_INTEL_RULE_VERSION = 'rel-intel/v1'

export interface PersonLookup {
  findExplicitSourceLink(
    source: string,
    sourceAccount: string,
    sourceIdentityKey: string,
  ): Promise<{ personId: string } | null>
  findPeopleByEmail(normalizedEmail: string): Promise<{ personId: string }[]>
  findPeopleByPhone(normalizedPhone: string): Promise<{ personId: string }[]>
}

function decision(
  reviewState: ReviewState,
  matchMethod: MatchMethod,
  matchConfidence: MatchConfidence,
  reason: string,
  canonicalPersonId: string | null,
): ReconcileDecision {
  return { reviewState, matchMethod, matchConfidence, canonicalPersonId, reason, ruleVersion: REL_INTEL_RULE_VERSION }
}

export async function reconcileEvidence(
  evidence: RelationshipEvidence,
  lookup: PersonLookup,
): Promise<ReconcileDecision> {
  // 1. Suppress automated/bulk and service/organization senders.
  if (evidence.isAutomatedOrBulk || evidence.isOrganizationOrService) {
    return evidence.isOrganizationOrService
      ? decision('non_person', 'rejected', 'none', 'automated_service_or_organization', null)
      : decision('rejected', 'rejected', 'none', 'automated_or_bulk_evidence', null)
  }

  // 2. Explicit source link (highest trust, never guessed).
  const link = await lookup.findExplicitSourceLink(
    evidence.source,
    evidence.sourceAccount,
    evidence.sourceIdentityKey,
  )
  if (link) {
    return decision('exact_linked', 'source_link', 'exact', 'explicit_source_link', link.personId)
  }

  // 3. Inspect ALL exact normalized email/phone ownership. Never resolve on the
  //    first match: evidence carrying multiple identities (e.g. email -> Person A
  //    and phone -> Person B) is a cross-identity conflict, never a silent choice
  //    based on lookup order.
  const emails = evidence.emails
    .map((e) => e.normalized)
    .filter((e): e is string => Boolean(e))
  const phones = evidence.phones
    .map((p) => p.normalized)
    .filter((p): p is string => Boolean(p))

  const distinctOwners = new Set<string>()
  let multiMatch = false
  let matchedMethod: 'exact_email' | 'exact_phone' | null = null

  for (const email of emails) {
    const people = await lookup.findPeopleByEmail(email)
    if (people.length > 1) multiMatch = true
    else if (people.length === 1) {
      distinctOwners.add(people[0].personId)
      matchedMethod = matchedMethod ?? 'exact_email'
    }
  }
  for (const phone of phones) {
    const people = await lookup.findPeopleByPhone(phone)
    if (people.length > 1) multiMatch = true
    else if (people.length === 1) {
      distinctOwners.add(people[0].personId)
      matchedMethod = matchedMethod ?? 'exact_phone'
    }
  }

  if (multiMatch) {
    return decision('ambiguous', matchedMethod ?? 'exact_email', 'ambiguous', 'identity_matches_multiple_people', null)
  }
  if (distinctOwners.size === 1 && matchedMethod) {
    const personId = [...distinctOwners][0]
    return decision('exact_linked', matchedMethod, 'exact', 'exact_normalized_identity', personId)
  }
  if (distinctOwners.size > 1) {
    return decision('ambiguous', matchedMethod ?? 'exact_email', 'ambiguous', 'cross_identity_conflict', null)
  }

  // 5. No exact match — classify conservatively.
  const hasUsableIdentity = evidence.hasEmail || evidence.hasPhone
  const hasMeaningfulEvidence = Boolean(
    evidence.isTwoWay ||
      evidence.isOwnerInitiated ||
      (evidence.outboundCount ?? 0) > 0,
  )
  if (!hasUsableIdentity) {
    return decision('deferred', 'unmatched', 'none', 'insufficient_identity_evidence', null)
  }
  if (hasMeaningfulEvidence) {
    return decision('review_required', 'review_candidate', 'probable', 'two_way_or_owner_initiated_without_exact_match', null)
  }
  return decision('unmatched', 'unmatched', 'none', 'no_exact_match', null)
}

/**
 * Deterministic batch reconciliation over many evidence rows. The matcher is
 * invoked once per row; the pass is rerunnable and reproducible for the same
 * inputs.
 */
export async function reconcileEvidenceBatch(
  rows: RelationshipEvidence[],
  lookup: PersonLookup,
): Promise<Map<string, ReconcileDecision>> {
  const out = new Map<string, ReconcileDecision>()
  for (const row of rows) {
    const key = `${row.source}\u0000${row.sourceAccount}\u0000${row.sourceIdentityKey}`
    out.set(key, await reconcileEvidence(row, lookup))
  }
  return out
}
