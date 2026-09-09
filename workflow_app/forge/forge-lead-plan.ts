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
// assess PRE-Smith. Absent or malformed is NO_PLAN (NOT a HOLD): this is
// behavior-preserving — real runs keep today's path until the contract is
// proven reliably emitted by dogfood, at which point the enqueue seam can be
// made authoritative (HOLD on absent plan when warranted).
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
      const outcome = typeof c.outcome === 'string' ? c.outcome.trim() : ''
      const surface = Array.isArray(c.surface)
        ? c.surface.filter((s): s is string => typeof s === 'string' && s.trim() !== '')
        : []
      const invariant = typeof c.invariant === 'string' ? c.invariant.trim() : ''
      const proof = typeof c.proof === 'string' ? c.proof.trim() : ''
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
 *  Returns hold reasons ([] = safe to hand off). Lead decisions that do NOT
 *  dispatch (SOLO/HOLD) and NO_PLAN (Lead emitted no LEAD_PLAN) never hold — the
 *  structured-plan contract is additive until dogfood proves Lead emits it
 *  reliably, at which point absent-plan policy can be tightened separately. */
export function leadPreDispatchHoldReasons(
  leadDecision: LeadDispatchDecision | undefined,
  notes: string | null | undefined,
): string[] {
  if (leadDecision !== 'SMITH' && leadDecision !== 'SPLIT') return []
  const gate = assessLeadPreDispatch(notes)
  if (gate.verdict !== 'HOLD') return []
  return gate.reasons.map((r) => `lead-plan:${r}`)
}
