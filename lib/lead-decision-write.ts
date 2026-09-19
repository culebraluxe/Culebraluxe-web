/**
 * HOW A LEAD DECISION IS ALLOWED TO BE WRITTEN (work package A).
 *
 * The refusals below used to live inline in `scripts/forge-handoff.mjs`, where the only way to exercise them was
 * to run the CLI — and the CLI refuses to run at all without a LIVE engine task, so the rule could not be tested
 * without an engine run. That is the shape of defect this repository keeps paying for: the production rule is
 * real, and nothing can demonstrate it. So the rule lives here, the CLI calls it, and the tests call it too.
 *
 * ORDER MATTERS AND IT IS THE POINT. The two flags are not independent values that happen to share a command:
 *   1. a decision outside the closed set is refused by the mediator (not here);
 *   2. ASSAY REQUIRES a candidate — "judge the work that already exists" names no work otherwise;
 *   3. a candidate with any other route is a MISMATCH — the proposal would describe two different routes;
 *   4. the candidate must be exactly 40 hex characters, because the validator compares it for equality.
 * Every refusal is returned with its own code, so the CLI can print the specific reason and a caller can assert
 * on it rather than on prose.
 */

export type LeadDecisionWrite =
  | { ok: true; decision: string; verifyCandidate: string | null }
  | { ok: false; code: 'ASSAY_REQUIRES_CANDIDATE' | 'CANDIDATE_REQUIRES_ASSAY' | 'CANDIDATE_NOT_A_SHA'; message: string }

export function validateLeadDecisionWrite(input: {
  decision: string
  /** The ALREADY-MEDIATED candidate (lowercased 40-hex) or null when the flag was not given. */
  verifyCandidate: string | null
}): LeadDecisionWrite {
  const { decision } = input
  const candidate = input.verifyCandidate

  if (decision === 'ASSAY' && candidate === null) {
    return {
      ok: false,
      code: 'ASSAY_REQUIRES_CANDIDATE',
      message:
        '--decision ASSAY requires --verify-candidate <40-hex sha>. The direct-to-QA route judges work that ' +
        'already exists, so the contract must name WHICH commit it verifies; an unnamed candidate is not a ' +
        'route. Nothing was written. Add the flag and run the command again.',
    }
  }

  if (candidate !== null && decision !== 'ASSAY') {
    return {
      ok: false,
      code: 'CANDIDATE_REQUIRES_ASSAY',
      message:
        `--verify-candidate is only meaningful with --decision ASSAY, and the decision is ${decision}. A ` +
        `${decision} route does not verify an existing candidate; naming one describes a different route than ` +
        'the one being recorded. Nothing was written.',
    }
  }

  if (candidate !== null && !/^[0-9a-f]{40}$/.test(candidate)) {
    return {
      ok: false,
      code: 'CANDIDATE_NOT_A_SHA',
      message:
        `--verify-candidate ${JSON.stringify(candidate)} is not a 40-hex sha. The direct-to-QA route names the ` +
        'exact commit it verifies, and a prefix or a malformed value describes no commit at all.',
    }
  }

  return { ok: true, decision, verifyCandidate: candidate }
}
