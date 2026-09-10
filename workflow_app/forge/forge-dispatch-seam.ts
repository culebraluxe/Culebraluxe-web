// ---------------------------------------------------------------------------
// forge-dispatch-seam — the RUNNING enforcement seam for Smith work-safety.
//
// The full KRAKEN gate (assessSmithDispatch in forge-dispatch-gate.ts) is
// test-only today: the only Smith work-safety check in the running path is an
// inline bounds test in agent-runtime-role-runner on Smith's self-reported
// SMITH_PLAN envelope {size, chunks, proofs}. That envelope is NOT a full
// SmithExecutionPlan (no per-chunk outcome/surface/invariant), so the full gate
// cannot score it structurally.
//
// This seam makes the gate AUTHORITATIVE for the enforcement that DOES exist:
// the runner's Smith work-safety check now delegates here — one adjudication
// path shared by the running path and tests — instead of an inline ad-hoc test.
// The seam is behavior-preserving for today's real capture (OVERSIZED or >3
// chunks => HOLD) and upgrades to the FULL gate whenever a full structured plan
// is ever available (the next expedition: Lead emitting a structured plan
// pre-Smith).
//
// Pure, DB-free, unit-testable.
// ---------------------------------------------------------------------------

import {
  type SmithPlan,
  parseSmithPlan,
  smithPlanExceedsBounds,
} from '../../agent-runtime/run-guardrails'
import {
  type SmithDispatchAssessment,
  assessSmithDispatch,
} from './forge-dispatch-gate'
import type { SmithExecutionPlan } from './forge-execution-shaping'

export type SmithWorkVerdict = 'GO' | 'HOLD'

export type SmithWorkAssessment = {
  verdict: SmithWorkVerdict
  reasons: string[]
  /**
   * Source of the adjudication:
   *   - 'full-dispatch'   the complete KRAKEN gate ran on a structured plan;
   *   - 'envelope-guard'  today's real capture (Smith's SMITH_PLAN envelope)
   *                       was adjudicated by the bounded envelope rules.
   */
  gate: 'full-dispatch' | 'envelope-guard'
  envelope: SmithPlan | null
  full?: SmithDispatchAssessment | null
}

/**
 * One enforcement seam for Smith work-safety. The running path and tests both
 * call this; the runner no longer holds an inline bounds test of its own.
 *
 * Behavior-preserving for what we capture today: no SMITH_PLAN line, or a plan
 * within bounds, dispatches (GO); an OVERSIZED plan or one claiming more than 3
 * chunks is a HOLD (a 4th chunk is not "keep working"). When a full structured
 * plan is supplied, the complete KRAKEN gate adjudicates (FLAG still dispatches,
 * per gate semantics — a FLAG only raises the proof requirement).
 */
export function assessSmithWork(
  raw: string | null | undefined,
  fullPlan?: SmithExecutionPlan | null,
): SmithWorkAssessment {
  const envelope = parseSmithPlan(raw)

  if (fullPlan) {
    const full = assessSmithDispatch(fullPlan)
    return {
      verdict: full.verdict === 'HOLD' ? 'HOLD' : 'GO',
      reasons: full.reasons,
      gate: 'full-dispatch',
      envelope,
      full,
    }
  }

  if (!envelope) {
    return { verdict: 'GO', reasons: [], gate: 'envelope-guard', envelope: null }
  }

  if (smithPlanExceedsBounds(envelope)) {
    return {
      verdict: 'HOLD',
      reasons: [
        `smith-plan:oversized (size=${envelope.size} chunks=${envelope.chunks}; >3 chunks is a HOLD, not keep working)`,
      ],
      gate: 'envelope-guard',
      envelope,
    }
  }

  return { verdict: 'GO', reasons: [], gate: 'envelope-guard', envelope }
}

/**
 * Render one Smith work-safety adjudication as a durable story-run detail line
 * (mirrors the `lead_pre` dispatch-gate record written by the runner). The seam
 * verdict/reasons remain the authority; this is observer-only text for audits.
 */
export function smithDispatchRunDetail(assessment: SmithWorkAssessment): string {
  const reasons =
    assessment.reasons.length > 0 ? assessment.reasons.join(' | ') : 'none'
  return (
    `dispatch gate node=smith verdict=${assessment.verdict} ` +
    `gate=${assessment.gate} reasons=${reasons}`
  )
}
