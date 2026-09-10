// ---------------------------------------------------------------------------
// forge-lead-plan — the Lead -> Smith structured-plan contract (expedition).
//
// Lead owns decomposition. Today Lead emits only a routing decision
// (SMITH/SPLIT/HOLD/SOLO) plus prose "Split lane N scope" lines — no machine
// plan — so the KRAKEN gate cannot assess Lead's decomposition PRE-Smith.
// Smith's later SMITH_PLAN is a reduced envelope (size/chunks/proofs) checked
// after the fact by forge-dispatch-seam.
//
// This module adds an OPTIONAL, additive machine line Lead may emit before any
// Smith token is spent:
//
//   LEAD_PLAN: {
//     "size": "SMALL|MEDIUM|LARGE",
//     "chunks": [ { "id":1, "outcome":"...", "surface":["..."],
//                  "invariant":"...", "proof":"pnpm exec tsx --test ...",
//                  "dependsOn":[1]? }, ... ]
//   }
//
// When present and valid it yields a full SmithExecutionPlan the gate can
// assess PRE-Smith. Absent is NO_PLAN. AUTHORITATIVE: for a Lead that decided
// to dispatch to Smith (SMITH/SPLIT), NO_PLAN is itself a HOLD back to Lead —
// the pre-Smith fuse must never be silently skipped because Lead emitted no
// plan.
//
// Pure, DB-free, unit-testable.
// ---------------------------------------------------------------------------

import type { SmithChunk, SmithExecutionPlan } from './forge-execution-shaping'
import {
  type SmithDispatchAssessment,
  assessSmithDispatch,
} from './forge-dispatch-gate'

export const LEAD_PLAN_PATTERN = /LEAD_PLAN:\s*\{/im

export type LeadPlan = SmithExecutionPlan

/** Parse Lead's optional LEAD_PLAN machine line into a full structured plan the
 *  KRAKEN gate can assess. Returns null when absent, malformed, or structurally
 *  unusable. Uses balanced-brace scanning because the JSON nests per-chunk
 *  objects (unlike the flat Smith SMITH_PLAN envelope). */
export function parseLeadPlan(notes: string | null | undefined): LeadPlan | null {
  if (!notes) return null
  const leadIdx = notes.search(LEAD_PLAN_PATTERN)
  if (leadIdx < 0) return null
  const start = notes.indexOf('{', leadIdx)
  if (start < 0) return null
  let depth = 0
  let end = -1
  for (let i = start; i < notes.length; i++) {
    const ch = notes[i]
    if (ch === '{') depth++
    else if (ch === '}') {
      depth--
      if (depth === 0) {
        end = i
        break
      }
    }
  }
  if (end < 0) return null
  const slice = notes.slice(start, end + 1)
  try {
    const raw = JSON.parse(slice) as {
      size?: unknown
      chunks?: unknown
    }
    const size = String(raw.size ?? '').toUpperCase()
    if (!['SMALL', 'MEDIUM', 'LARGE'].includes(size)) return null
    if (!Array.isArray(raw.chunks) || raw.chunks.length === 0) return null

    const chunks: SmithChunk[] = []
    for (const entry of raw.chunks) {
      const c = entry as Record<string, unknown>
      const id = Number(c.id)
      // Canonical work-order vocabulary (scope/acceptance/preconditions/
      // postconditions) with fallback to the original keys (outcome/surface/
      // invariant/proof). The operator-facing work order maps onto the plan:
      //   scope -> surface | acceptance -> proof | postconditions -> invariant
      const outcome =
        (typeof c.outcome === 'string' ? c.outcome.trim() : '') ||
        (typeof c.postconditions === 'string' ? c.postconditions.trim() : '')
      const rawSurface = Array.isArray(c.surface) ? c.surface : c.scope
      const surface = Array.isArray(rawSurface)
        ? rawSurface.filter((s): s is string => typeof s === 'string' && s.trim() !== '')
        : []
      const invariant =
        (typeof c.invariant === 'string' ? c.invariant.trim() : '') ||
        (typeof c.postconditions === 'string' ? c.postconditions.trim() : '')
      const proof =
        (typeof c.proof === 'string' ? c.proof.trim() : '') ||
        (typeof c.acceptance === 'string' ? c.acceptance.trim() : '')
      if (!Number.isInteger(id) || id < 1 || !outcome || surface.length === 0 || !invariant || !proof) {
        return null
      }
      const dependsOn = Array.isArray(c.dependsOn)
        ? c.dependsOn.map((d) => Number(d)).filter((d) => Number.isInteger(d) && d >= 1)
        : []
      chunks.push({
        id,
        outcome,
        surface,
        invariant,
        proof,
        ...(dependsOn.length > 0 ? { dependsOn } : {}),
      })
    }
    return { size: size as SmithExecutionPlan['size'], chunks }
  } catch {
    return null
  }
}

export type LeadPreDispatchVerdict = 'GO' | 'FLAG' | 'HOLD' | 'NO_PLAN'

export type LeadPreDispatchAssessment = {
  /** Whether Lead actually emitted a parseable LEAD_PLAN. */
  planPresent: boolean
  plan: LeadPlan | null
  verdict: LeadPreDispatchVerdict
  reasons: string[]
  full?: SmithDispatchAssessment | null
}

/** The pre-Smith gate seam. When Lead emitted a valid structured plan, the FULL
 *  KRAKEN gate adjudicates it (GO/FLAG dispatch, HOLD blocks). When no plan was
 *  emitted, returns NO_PLAN — behavior-preserving today; the enqueue seam can
 *  later choose to HOLD on absent plans once the contract is proven reliable. */
export function assessLeadPreDispatch(
  notes: string | null | undefined,
): LeadPreDispatchAssessment {
  const plan = parseLeadPlan(notes)
  if (!plan) {
    return { planPresent: false, plan: null, verdict: 'NO_PLAN', reasons: [] }
  }
  const full = assessSmithDispatch(plan)
  return { planPresent: true, plan, verdict: full.verdict, reasons: full.reasons, full }
}

export type LeadDispatchDecision = 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD'

/** The running handoff gate for lead_pre: does this Lead handoff that decided to
 *  dispatch to Smith (SMITH/SPLIT) have to be HELD BEFORE any Smith lane starts?
 *  Returns hold reasons ([] = safe to hand off).
 *
 *  AUTHORITATIVE: a SMITH/SPLIT dispatch may NOT start Smith unless Lead emitted
 *  an assessable LEAD_PLAN. Missing plan (NO_PLAN) is itself a HOLD back to Lead
 *  — the pre-Smith fuse must not be silently skipped just because Lead did not
 *  produce a plan. Decisions that do NOT dispatch (SOLO/HOLD) are not gated. */
export function leadPreDispatchHoldReasons(
  leadDecision: LeadDispatchDecision | undefined,
  notes: string | null | undefined,
): string[] {
  if (leadDecision !== 'SMITH' && leadDecision !== 'SPLIT') return []
  const gate = assessLeadPreDispatch(notes)
  if (gate.verdict === 'HOLD') return gate.reasons.map((r) => `lead-plan:${r}`)
  if (gate.verdict === 'NO_PLAN') {
    return ['lead-plan:missing (SMITH/SPLIT requires an assessable LEAD_PLAN before Smith starts)']
  }
  return []
}

export type LeadHandoffGate = {
  /** True only on the Lead pre-dispatch handoff node. */
  applies: boolean
  /** Hold reasons ([] = safe to hand off, or not gated at all). */
  holds: string[]
  verdict: 'GO' | 'HOLD' | 'NOT_GATED'
}

/**
 * The RUNNER's decision, as a pure function: may this node hand off to Smith?
 *
 * This is the call the role runner makes on the `lead_pre` node. Keeping it pure
 * means the running path's hold decision is asserted by tests directly instead of
 * being re-derived from reading the runner — the review's ask: "a Lead that
 * routes SMITH with no parseable LEAD_PLAN must not start a Smith adapter."
 *
 * Scoping matters: the gate applies ONLY at the Lead handoff. It is not applied
 * on a `smith` node (Smith has already run there, so holding would be the
 * after-the-fact behaviour we are eliminating).
 */
export function assessLeadHandoff(
  nodeId: string,
  leadDecision: LeadDispatchDecision | undefined,
  notes: string | null | undefined,
): LeadHandoffGate {
  if (nodeId !== 'lead_pre') return { applies: false, holds: [], verdict: 'NOT_GATED' }
  const holds = leadPreDispatchHoldReasons(leadDecision, notes)
  return { applies: true, holds, verdict: holds.length > 0 ? 'HOLD' : 'GO' }
}

/** Render Lead's plan as explicit per-chunk WORK ORDERS for Smith (Phase 3).
 *  Smith executes these verbatim — it does NOT re-plan or enlarge scope. */
export function renderSmithWorkOrders(plan: LeadPlan): string {
  const lines = [
    'WORK ORDERS from Lead (execute these chunks serially in order; do NOT re-plan and do NOT enlarge scope):',
  ]
  for (const chunk of plan.chunks) {
    lines.push(`Chunk ${chunk.id}:`)
    lines.push(`  Scope: ${chunk.surface.join(', ')}`)
    if (chunk.dependsOn && chunk.dependsOn.length > 0) {
      lines.push(`  Preconditions: depends on chunk ${chunk.dependsOn.join(', ')}`)
    }
    lines.push(`  Acceptance (runnable, write the test first unless it reuses an existing one): ${chunk.proof}`)
    lines.push(`  Postcondition (must still hold after): ${chunk.invariant}`)
  }
  return lines.join('\n')
}

/** Find the latest Lead-pre plan in a story run list (falls back over older runs). */
export function findLatestLeadPlan(
  runs: Array<{ runType?: string | null; notes?: string | null }> | null | undefined,
): LeadPlan | null {
  if (!runs || runs.length === 0) return null
  for (let i = runs.length - 1; i >= 0; i--) {
    const run = runs[i]
    if (run.runType === 'lead' && (run.notes ?? '').includes('LEAD_PLAN')) {
      const plan = parseLeadPlan(run.notes)
      if (plan) return plan
    }
  }
  return null
}
