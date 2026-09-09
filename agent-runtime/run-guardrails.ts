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

/**
 * SMITH WORK DECOMPOSITION — the bounded anti-token-burn contract. Injected into
 * every Smith run so it sizes the Architect assignment by COUPLING, UNCERTAINTY,
 * CHANGE SURFACE, and PROOF BURDEN (never file count / LOC), emits a <=3 serial
 * chunk plan with a targeted proof per chunk, and HOLDs instead of disappearing
 * when the work genuinely exceeds 3 chunks or the scope expands. This recreates a
 * human lead's stopping points automatically: chunk-complete -> proof -> scope
 * still bounded? -> continue | HOLD.
 */
export function buildSmithWorkDecompositionDirective(): string {
  return [
    'SMITH WORK DECOMPOSITION - size the Architect assignment BEFORE editing code.',
    'Size by COUPLING (what must be correct together), UNCERTAINTY (what you must still discover), CHANGE SURFACE (distinct architectural areas), and PROOF BURDEN (independent behaviors to verify). NEVER by file count or LOC alone.',
    'Classify: SMALL = 1 coherent path + 1 proof boundary. MEDIUM = 2 meaningful dependency/proof boundaries. LARGE = 3 boundaries. OVERSIZED = cannot be expressed as <=3 coherent chunks, or contains multiple independent business outcomes.',
    'Before editing, emit exactly one machine line: SMITH_PLAN: {"size":"SMALL|MEDIUM|LARGE","chunks":1|2|3,"proofs":["<chunk1 one-line proof>",...]}.',
    'Execute chunks SERIALLY in this same session and same authoritative worktree. Preserve accumulated investigation context between chunks - do not re-derive.',
    'A valid chunk has ONE clear outcome, a reason it occurs at that point in dependency order, an identifiable code/symbol surface, one invariant it establishes, and a targeted proof.',
    'After EACH chunk: (1) run its targeted proof, (2) record what you learned, (3) reassess remaining chunks against actual repo facts, (4) continue only if the original story is still bounded.',
    'If the work cannot be expressed as <=3 coherent chunks, or implementing reveals material scope expansion (a 4th chunk, a new independent outcome, or an Architect-contract change): STOP and HOLD with your proposed story decomposition. Do NOT burn tokens expanding the assignment.',
    'Chunks need not be equal. Prefer risk-first / dependency-first. The goal is the FEWEST safe proof boundaries that complete the story without losing control of scope.',
  ].join('\n')
}

/** Machine line Smith emits before editing, so its size/chunk claim is capturable
 * for the empirical threshold-learning data lake (never the enforcement gate). */
export const SMITH_PLAN_PATTERN = /SMITH_PLAN:\s*\{[^}]*\}/im

export type SmithPlanSize = 'SMALL' | 'MEDIUM' | 'LARGE' | 'OVERSIZED'

export type SmithPlan = {
  size: SmithPlanSize
  chunks: number
  proofs: string[]
}

/** Parse Smith's emitted SMITH_PLAN line (data capture for later threshold
 * learning, not a gate). Returns null when absent or malformed. */
export function parseSmithPlan(notes: string | null | undefined): SmithPlan | null {
  if (!notes) return null
  const m = notes.match(SMITH_PLAN_PATTERN)
  if (!m) return null
  try {
    const raw = JSON.parse(m[0].replace(/^SMITH_PLAN:\s*/, '')) as {
      size?: unknown
      chunks?: unknown
      proofs?: unknown
    }
    const size = String(raw.size ?? '').toUpperCase()
    if (!['SMALL', 'MEDIUM', 'LARGE', 'OVERSIZED'].includes(size)) return null
    const chunks = Number(raw.chunks)
    const proofs = Array.isArray(raw.proofs)
      ? raw.proofs.filter((p): p is string => typeof p === 'string')
      : []
    return {
      size: size as SmithPlanSize,
      chunks: Number.isInteger(chunks) ? chunks : 0,
      proofs,
    }
  } catch {
    return null
  }
}

/** True when a Smith plan is OVERSIZED or claims more than 3 chunks — the
 * bounded fail-fast signal that the story belongs back with Lead, not ground in
 * Smith. Pure advisory for data + instruction; not a gate. */
export function smithPlanExceedsBounds(plan: SmithPlan | null): boolean {
  if (!plan) return false
  return plan.size === 'OVERSIZED' || plan.chunks > 3
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
