// -----------------------------------------------------------------------------
// Run guardrails — spend/stop lines injected into EVERY model-backed role's
// prompt (scout, architect, lead, smith), not just Lead.
//
// Cline-vs-OpenCode quality is envelope, not brain. The single biggest missing
// control is a hard stop: a one-shot headless run must know it has a budget,
// must not brute-force, and must STOP to HOLD when a job is ambiguous/oversized.
// -----------------------------------------------------------------------------
import { accessSync, constants } from 'node:fs'
import { join } from 'node:path'
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

/**
 * Grounding directive for JUDGMENT roles (architect/lead). The exact burn we saw
 * was ungrounded judgment turning into repo-search loops. Scout is the searcher;
 * architect/lead must answer from the provided context ONLY, and STOP if the
 * grounding is insufficient — never go turn-search the repository.
 */
export function buildGroundingDirective(): string {
  return [
    'You are a JUDGMENT role (architect/lead). Answer from the PROVIDED context only: the scout research, story packet, and repo-context handed to you.',
    'Do NOT run repo scans, broad searches, or exploratory tool turns to go find the answer. Grounding is a Scout responsibility, not yours.',
    'If the provided grounding is insufficient to decide soundly, do NOT go searching. State that the grounding is insufficient and choose the HOLD/STOP disposition so a human/Scout can complete it.',
  ].join('\n')
}

/** Resolve the `rtk` context-compressor binary from env (override RTK_BIN wins,
 * else a PATH scan). Returns null when it is not installed — callers must never
 * claim rtk ran when it is absent. */
export function resolveRtkBin(env: NodeJS.ProcessEnv = process.env): string | null {
  const override = (env.RTK_BIN ?? '').trim()
  if (override) return override
  const pathDirs = (env.PATH ?? '').split(':').filter(Boolean)
  for (const dir of pathDirs) {
    const candidate = join(dir, 'rtk')
    try {
      accessSync(candidate, constants.X_OK)
      return candidate
    } catch {
      /* not here */
    }
  }
  return null
}

/** Whether a context compressor is available to this run. */
export function rtkAvailable(env: NodeJS.ProcessEnv = process.env): boolean {
  return resolveRtkBin(env) !== null
}

/**
 * Context-compression directive. When `rtk` is installed it is injected into the
 * run so every role routes LARGE command output through it and keeps its context
 * bounded (the lever that decides how much a lane can carry). Returns null when
 * rtk is absent so the run never claims a compressor that is not there.
 */
export function buildRtkCompressionDirective(
  env: NodeJS.ProcessEnv = process.env,
): string | null {
  const bin = resolveRtkBin(env)
  if (!bin) return null
  return [
    `A context compressor (\`rtk\` at ${bin}) is available in this run.`,
    'Route LARGE command output through it so your context stays bounded: `rtk read <file>`, `rtk tree`/`rtk ls`, `rtk git`/`rtk diff`/`rtk log`, `rtk test` (failures only), `rtk build`, `rtk json`, `rtk deps`, `rtk err <cmd>` (errors/warnings only).',
    'Never dump a large raw command transcript into context when the `rtk` variant returns the compact slice.',
  ].join('\n')
}
