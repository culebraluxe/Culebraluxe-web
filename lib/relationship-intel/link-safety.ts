// ---------------------------------------------------------------------------
// REL-INTEL — canonical link safety (match-once / enrich-forever).
//
// A durable source identity that has a non-null canonical_person_id is owned by
// that canonical Person for good. Automated reconciliation may ENRICH that
// Person, but it may NEVER re-decide who the Person is: it cannot clear the
// link, redirect it to another Person, merge Persons, or steal an identity.
//
// This pure function is the SINGLE source of truth for that invariant. The
// write seam (db/relationship-evidence.ts recordReconcileDecision) reads the
// current link, runs this merge, and persists exactly what it returns.
// ---------------------------------------------------------------------------

import type {
  MatchConfidence,
  MatchMethod,
  ReconcileDecision,
  ReviewState,
} from './contracts'

/** The safe persistence target for one reconciliation decision. */
export type SafeReconcileWrite = {
  canonicalPersonId: string | null
  reviewState: ReviewState
  matchMethod: MatchMethod
  matchConfidence: MatchConfidence
  reason: string
  /** True when an automated decision conflicted with a durable link. */
  conflictSurfaced: boolean
}

/** Reason recorded when an automated decision would have cleared/redirected a link. */
export const ESTABLISHED_LINK_CONFLICT_REASON = 'established_link_preserved_automated_conflict'

/**
 * Merge a fresh automated reconciliation decision against the current durable
 * canonical link for a source identity.
 *
 *   - no existing link  -> establish (or leave null)  [CASE A]
 *   - same Person       -> preserve                   [CASE B]
 *   - decision clears   -> preserve current           [CASE C]
 *   - decision redirects-> preserve current + surface [CASE D]
 *
 * The established link is durable state. Never cleared, never redirected, never
 * merged.
 */
export function mergeReconcileDecision(
  currentCanonicalPersonId: string | null,
  decision: ReconcileDecision,
): SafeReconcileWrite {
  const incomingPersonId = decision.canonicalPersonId ?? null

  // No established link -> follow the normal reconciliation path (establish once).
  if (currentCanonicalPersonId == null) {
    return {
      canonicalPersonId: incomingPersonId,
      reviewState: decision.reviewState,
      matchMethod: decision.matchMethod,
      matchConfidence: decision.matchConfidence,
      reason: decision.reason,
      conflictSurfaced: false,
    }
  }

  // An automated decision suggesting a DIFFERENT Person: preserve current and
  // surface the conflict. Never switch, never clear, never merge.
  if (incomingPersonId != null && incomingPersonId !== currentCanonicalPersonId) {
    return {
      canonicalPersonId: currentCanonicalPersonId,
      reviewState: 'exact_linked',
      matchMethod: decision.matchMethod,
      matchConfidence: 'ambiguous',
      reason: ESTABLISHED_LINK_CONFLICT_REASON,
      conflictSurfaced: true,
    }
  }

  // Same Person (replay) or an automated decision that would clear the link
  // (null): both preserve the established link. A clear attempt is surfaced.
  const preserved =
    incomingPersonId === currentCanonicalPersonId && incomingPersonId != null
  return {
    canonicalPersonId: currentCanonicalPersonId,
    reviewState: 'exact_linked',
    matchMethod: decision.matchMethod,
    matchConfidence: preserved ? decision.matchConfidence : 'ambiguous',
    reason: preserved ? decision.reason : ESTABLISHED_LINK_CONFLICT_REASON,
    conflictSurfaced: !preserved,
  }
}
