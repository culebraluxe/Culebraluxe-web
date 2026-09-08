// -----------------------------------------------------------------------------
// Run guardrails — spend/stop lines injected into EVERY model-backed role's
// prompt (scout, architect, lead, smith), not just Lead.
//
// Cline-vs-OpenCode quality is envelope, not brain. The single biggest missing
// control is a hard stop: a one-shot headless run must know it has a budget,
// must not brute-force, and must STOP to HOLD when a job is ambiguous/oversized.
// -----------------------------------------------------------------------------
import { runWallClockBudgetMs } from './agent-runtime-adapter'
import { renderModelCostLines } from './model-prices'

export function buildRunGuardrailsDirective(env: NodeJS.ProcessEnv = process.env): string {
  const minutes = Math.max(1, Math.round(runWallClockBudgetMs(env) / 60000))
  return [
    ...renderModelCostLines(),
    `Your run has a HARD ${minutes}-minute wall-clock budget; the runner cancels it automatically at that limit.`,
    'Do NOT brute-force or wander. Produce the bounded deliverable for THIS node only, then STOP.',
    'If the work is ambiguous, oversized, or would clearly exceed the budget, do NOT grind. Report the blocker and choose the HOLD/stop disposition so a human decides.',
  ].join('\n')
}

/** Bounded autonomy: N passes per run (1 = one pass then stop for review). */
export function runPassBudget(env: NodeJS.ProcessEnv = process.env): number {
  const n = Number((env.FORGE_RUN_PASSES ?? '').trim())
  return Number.isInteger(n) && n >= 1 ? n : 1
}

export type RunPassStop = 'COMPLETE' | 'NEEDS_REVIEW' | 'NEEDS_MORE_PASSES'

export const PASS_STOP_PATTERN = /^\s*FORGE_PASS_STOP:\s*(COMPLETE|NEEDS_REVIEW|NEEDS_MORE_PASSES)\.?\s*$/im

/** Parse the machine stop disposition a run emits at the end of a pass. */
export function parseRunPassStop(notes: string | null | undefined): RunPassStop | null {
  if (!notes) return null
  const match = notes.match(PASS_STOP_PATTERN)
  return (match?.[1] as RunPassStop | undefined) ?? null
}

/**
 * One-bounded-pass operating mode. A run may take up to FORGE_RUN_PASSES passes,
 * but each pass must STOP to a review checkpoint — never attempt the whole job
 * in one unbounded shot. Emitting a stop disposition lets the engine decide:
 * park for morning review (NEEDS_REVIEW), accept (COMPLETE), or schedule a
 * bounded follow-up pass (NEEDS_MORE_PASSES) instead of one Mount-Everest run.
 */
export function buildRunPassDirective(env: NodeJS.ProcessEnv = process.env): string {
  const passes = runPassBudget(env)
  return [
    `Bounded autonomy: execute at most ${passes} pass(es) per run; each pass must STOP to a review checkpoint.`,
    'A pass is a self-contained attempt that (a) produces the exact deliverable with evidence, or (b) reaches a clear stopping point. NEVER chain into a second pass silently — if you need another pass, end with NEEDS_MORE_PASSES and the engine decides.',
    'You are allowed to do real, scoped work — but if you start drifting into unrelated territory or an oversized rework, STOP immediately.',
    'End your run with exactly one machine line: FORGE_PASS_STOP: COMPLETE | NEEDS_REVIEW | NEEDS_MORE_PASSES',
  ].join('\n')
}