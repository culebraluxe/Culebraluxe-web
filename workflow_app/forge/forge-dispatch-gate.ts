// ---------------------------------------------------------------------------
// forge-dispatch-gate — the KRAKEN: one decision function that unleashes the
// whole policy toolkit before Smith. Given a Lead-emitted SmithExecutionPlan
// (and, when available, the Lead's qualitative judgment features), return a
// single GO / FLAG / HOLD so the executor can block NOT_DISPATCHABLE / reject
// work BEFORE any Smith token is spent.
//
// Combines, in order:
//   1. structural rules   (forge-execution-shaping): <=3 chunks, surfaces, proofs
//   2. qualitative        (forge-dispatchability):  Lead-judgment feature vector
//   3. difficulty         (forge-difficulty-scorer via forge-plan-difficulty):
//                          plan-derivable P(success) + reject/flag/dispatch
// A single call site can then enforce it. This is the enforcement seam.
// ---------------------------------------------------------------------------

import {
  type DispatchabilityFeatures,
  type DispatchabilityResult,
  dispatchabilityFor,
} from './forge-dispatchability'
import {
  type SmithExecutionPlan,
  validateSmithExecutionPlan,
} from './forge-execution-shaping'
import { scorePlan } from './forge-plan-difficulty'
import { type DifficultyScorer, LogisticScorer } from './forge-difficulty-scorer'

export type SmithDispatchVerdict = 'GO' | 'FLAG' | 'HOLD'

export type SmithDispatchAssessment = {
  verdict: SmithDispatchVerdict
  reasons: string[]
  structuralOk: boolean
  qualitative: DispatchabilityResult | null
  difficulty: { pSuccess: number; gate: 'reject' | 'flag' | 'dispatch' } | null
}

export type AssessDispatchOptions = {
  /** Lead's qualitative judgment features, when the Lead emitted them. */
  qualitativeFeatures?: DispatchabilityFeatures
  scorer?: DifficultyScorer
}

/** One consolidated pre-Smith gate. HOLD comes only from structural violations or
 *  qualitative NOT_DISPATCHABLE (the 6-surface monster). Difficulty P(success) is
 *  ADVISORY: reject and flag both surface as a FLAG (stronger proof required),
 *  never a HOLD, until the logistic scorer is calibrated on real runs. */
export function assessSmithDispatch(
  plan: SmithExecutionPlan | null | undefined,
  options: AssessDispatchOptions = {},
): SmithDispatchAssessment {
  const reasons: string[] = []
  const scorer = options.scorer ?? new LogisticScorer()

  const structural = validateSmithExecutionPlan(plan)
  const structuralOk = structural.length === 0
  if (!plan) return { verdict: 'HOLD', reasons: ['missing plan'], structuralOk: false, qualitative: null, difficulty: null }
  reasons.push(...structural)

  const qualitative = options.qualitativeFeatures
    ? dispatchabilityFor(options.qualitativeFeatures)
    : null
  if (qualitative && qualitative.verdict === 'NOT_DISPATCHABLE') {
    reasons.push(...qualitative.reasons.map((r) => `qualitative: ${r}`))
  }

  const scored = scorePlan(plan, scorer)
  const difficulty = { pSuccess: scored.pSuccess, gate: scored.gate }
  // Difficulty (the logistic P(success)) is ADVISORY only: the scorer is not yet
  // calibrated on real runs, so it must never be a HOLD authority. reject/flag
  // both surface as a FLAG (stronger proof), never a HOLD. Structural rules and
  // qualitative NOT_DISPATCHABLE are the only HOLD authorities (calibration
  // fiction guard — do not teach uncalibrated numbers to spend money).
  if (scored.gate === 'flag') reasons.push(`difficulty: flagged P(success)=${scored.pSuccess.toFixed(3)} (stronger proof required)`)
  if (scored.gate === 'reject') {
    reasons.push(
      `difficulty: advisory P(success)=${scored.pSuccess.toFixed(3)} below dispatch — scorer not calibrated on real runs; FLAG, not HOLD`,
    )
  }

  const holds = structural.length > 0 || qualitative?.verdict === 'NOT_DISPATCHABLE'
  const verdict: SmithDispatchVerdict = holds ? 'HOLD' : scored.gate === 'dispatch' ? 'GO' : 'FLAG'
  return { verdict, reasons, structuralOk, qualitative, difficulty }
}
