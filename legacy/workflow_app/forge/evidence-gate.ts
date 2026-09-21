// ---------------------------------------------------------------------------
// ENG-FORGE-HARDEN-03 — QA failure is repair: the hard candidate evidence gate.
//
// Core invariants (no evidence -> no promotion; failed evidence -> repair; only
// QA PASS against the EXACT frozen candidate SHA -> DEV_OPS-eligible):
//
//   - QA may only PASS a specific candidate SHA.
//   - A QA result for candidate A can never certify candidate B.
//   - FAIL leaves the story unshipped (never Complete, never DEV_OPS, never
//     grow/follow-on work as though shipped).
//   - Missing evidence is incomplete, never a PASS.
//
// This is pure and DB-free; it composes with the exact-lineage enforcement the
// FORGE_SDLC gate facts already apply (forge-facts.projectForgeGateFacts) rather
// than introducing a competing check.
// ---------------------------------------------------------------------------

export type CandidateEvidence = {
  /** The candidate SHA this QA evidence evaluated. */
  evaluatedSha: string | null
  qaVerdict: 'PASS' | 'FAIL'
  /** Authoritative verified SHA required for a PASS (never agent prose). */
  verifiedSha: string | null
  /** True only when real verification evidence exists (an anchor), not prose. */
  evidencePresent: boolean
}

export type EvidenceGateResult = {
  eligible: boolean
  blockers: string[]
}

/** Deterministically produce PASS evidence bound to an exact verified SHA. */
export function qaPass(candidateSha: string, verifiedSha: string): CandidateEvidence {
  return { evaluatedSha: candidateSha, qaVerdict: 'PASS', verifiedSha, evidencePresent: true }
}

/** Deterministically produce FAIL evidence. */
export function qaFail(candidateSha: string, note?: string): CandidateEvidence {
  return { evaluatedSha: candidateSha, qaVerdict: 'FAIL', verifiedSha: null, evidencePresent: true }
}

/** No evidence at all (an anchor is missing) -> incomplete, never PASS. */
export function noEvidence(): CandidateEvidence {
  return { evaluatedSha: null, qaVerdict: 'FAIL', verifiedSha: null, evidencePresent: false }
}

/**
 * Is the given candidate eligible for promotion? True only when QA PASSED with
 * real evidence verifying EXACTLY this candidate's SHA. A QA result for any
 * other SHA, a FAIL, or missing evidence blocks promotion (never silently pass).
 */
export function promotionEligibility(input: {
  candidateSha: string | null
  evidence: CandidateEvidence
}): EvidenceGateResult {
  const blockers: string[] = []
  if (!input.candidateSha) blockers.push('NO_CANDIDATE')
  if (!input.evidence.evidencePresent) blockers.push('NO_ANCHOR_EVIDENCE')
  if (input.evidence.qaVerdict === 'FAIL' && input.evidence.evidencePresent) {
    blockers.push('QA_FAIL')
  }
  if (input.evidence.qaVerdict === 'PASS' && !input.evidence.verifiedSha) {
    blockers.push('MISSING_VERIFIED_SHA')
  }
  // Candidate-SHA bound: an approval for A cannot certify B.
  if (
    input.evidence.evidencePresent &&
    input.evidence.evaluatedSha &&
    input.candidateSha &&
    input.evidence.evaluatedSha !== input.candidateSha
  ) {
    blockers.push('STALE_APPROVAL_SHA')
  }
  if (
    input.evidence.verifiedSha &&
    input.candidateSha &&
    input.evidence.verifiedSha !== input.candidateSha
  ) {
    blockers.push('VERIFIED_SHA_MISMATCH')
  }
  return { eligible: blockers.length === 0, blockers }
}
