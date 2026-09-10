// ---------------------------------------------------------------------------
// forge-dispatch-seam — the RUNNING enforcement seam for Smith work-safety.
//
// The full KRAKEN gate (assessSmithDispatch in forge-dispatch-gate.ts) is NO
// LONGER test-only. It runs in the live path PRE-Smith: the `lead_pre` handoff
// (forge-lead-plan.ts → assessLeadPreDispatch → assessSmithDispatch) adjudicates
// Lead's structured LEAD_PLAN before any Smith lane can start, and a missing or
// malformed plan is itself a HOLD (see assessLeadHandoff). That is the
// authoritative pre-Smith fuse.
//
// This seam remains the POST-Smith envelope guard: the runner adjudicates
// Smith's self-reported SMITH_PLAN envelope {size, chunks, proofs}. That envelope
// is not a full SmithExecutionPlan (no per-chunk outcome/surface/invariant), so
// the structural gate cannot score it — the seam applies the bounded envelope
// rules (OVERSIZED or >3 chunks => HOLD) and upgrades to the FULL gate whenever a
// full structured plan is supplied.
//
// One adjudication path shared by the running path and tests.
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
