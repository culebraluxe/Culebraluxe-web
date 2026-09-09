// ---------------------------------------------------------------------------
// forge-execution-shaping — the policy-layer marriage at the Lead -> Smith
// boundary. NOT a system: no agent, node, scheduler, DB, worktree or session is
// added. Forge stays the operating system and remains authoritative for
// execution, state, sessions, worktrees, verification and promotion.
//
// This layer ONLY shapes the WORK handed to the resident Smith: a small typed
// SmithExecutionPlan contract + a structural validator enforcing the hard rules
// (Stage 1 shadow / Stage 2 structural). Serial chunks run in the SAME Smith
// brain, SAME OpenCode session, SAME worktree — work units are separated without
// separating understanding.
// ---------------------------------------------------------------------------

import { ANTI_TOKEN_FIRE } from './forge-dispatchability'

export type SmithExecutionPlanSize = 'SMALL' | 'MEDIUM' | 'LARGE'

export type SmithChunk = {
  /** 1-based serial chunk id. */
  id: number
  /** ONE clear outcome for this chunk (never more). */
  outcome: string
  /** Exact owning code/symbol surfaces this chunk edits. */
  surface: string[]
  /** Prior chunk ids this chunk depends on (normally just id-1). */
  dependsOn?: number[]
  /** The invariant this chunk establishes. */
  invariant: string
  /** The runnable targeted proof that the chunk is done. */
  proof: string
}

/** The critical contract: Lead owns decomposition; Smith may refine inside a
 * chunk but may NOT enlarge story scope. 1..3 chunks only. */
export type SmithExecutionPlan = {
  size: SmithExecutionPlanSize
  chunks: SmithChunk[]
}

/** A structural violation of the hard rules (Stage 2). Non-empty => HOLD. */
export type SmithPlanViolation = string

/**
 * Validate a proposed Smith execution plan against the HARD structural rules —
 * the ones we are confident are universally sane and can be enforced in code:
 *   - 1..MAX_CHUNKS_PER_STORY chunks (a 4th chunk = HOLD, not keep working);
 *   - every chunk has exactly one outcome, an identifiable code surface,
 *     an invariant, and a runnable targeted proof;
 *   - serial chunk ids 1..n with no forward/gap dependencies.
 * Returns [] when structurally sound (the plan may still be rejected later by the
 * dispatchability scorer, with Lead override per the staging).
 */
export function validateSmithExecutionPlan(plan: SmithExecutionPlan | null | undefined): SmithPlanViolation[] {
  const violations: SmithPlanViolation[] = []
  if (!plan || !Array.isArray(plan.chunks)) return ['missing smith_execution_plan']

  const n = plan.chunks.length
  if (n < 1) violations.push('plan declares no chunks')
  if (n > ANTI_TOKEN_FIRE.MAX_CHUNKS_PER_STORY) {
    violations.push(
      `more than ${ANTI_TOKEN_FIRE.MAX_CHUNKS_PER_STORY} chunks (${n}): a 4th chunk is HOLD, not keep working`,
    )
  }
  if (!['SMALL', 'MEDIUM', 'LARGE'].includes(plan.size)) {
    violations.push(`unknown size '${String(plan.size)}' (expected SMALL|MEDIUM|LARGE)`)
  }

  plan.chunks.forEach((chunk) => {
    const label = `chunk ${chunk.id}: `
    if (!chunk.outcome || !String(chunk.outcome).trim()) violations.push(`${label}missing one outcome`)
    if (!Array.isArray(chunk.surface) || chunk.surface.length === 0) {
      violations.push(`${label}missing code surface`)
    } else if (chunk.surface.some((s) => !s || !String(s).trim())) {
      violations.push(`${label}has a blank code surface entry`)
    }
    if (!chunk.invariant || !String(chunk.invariant).trim()) violations.push(`${label}missing invariant`)
    if (!chunk.proof || !String(chunk.proof).trim()) violations.push(`${label}missing runnable targeted proof`)
  })

  // Serial ids 1..n, no gaps, no forward deps.
  const ids = plan.chunks.map((c) => c.id)
  for (let i = 0; i < ids.length; i++) {
    if (ids[i] !== i + 1) violations.push(`chunk ids must be serial 1..n (got ${ids.join(',')})`)
  }
  plan.chunks.forEach((chunk) => {
    const deps = (chunk.dependsOn ?? []).filter((d) => d !== chunk.id - 1)
    if (deps.length > 0) violations.push(`chunk ${chunk.id} has non-serial dependency (${deps.join(',')})`)
  })

  return violations
}
