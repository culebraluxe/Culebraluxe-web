/**
 * THE TURN BUDGET — how long a generation is allowed to stay interesting.
 *
 * MAP (arXiv 2512.04123, "Measuring Agents in Production") surveyed practitioners and found
 * 68% cap a workflow at TEN MODEL STEPS before a human is involved, about half at five, and
 * ~80% run a predefined workflow rather than open-ended planning. Their conclusion is that
 * production reliability is a harness/loop/graph property, not a model-intelligence one:
 * agents work because they are SHORT, STRUCTURED and BOXED.
 *
 * This is that cap, made visible as one integer per generation and enforced before the next
 * turn is dispatched. Measured on our own engine the same day: a healthy FEATURE generation
 * costs 5 turns (architect, lead_pre, smith, post, qa), a FAST one with a repair costs 4, and
 * a generation that HOLDs at the Lead costs 2. Ten therefore leaves a full retry-and-repair
 * of headroom while refusing the loop MAP's respondents were fleeing.
 *
 * WHAT THE CAP IS NOT: a model-quality judgement, a score, or a reason to re-try. A
 * generation that reaches it has already spent more turns than 68% of surveyed production
 * agents ever do, so the honest next actor is a human at the ENGINE QUEUE, not the same
 * model with another turn.
 *
 * Pure by contract: no database, no process reads at module top level, so the rule can be
 * tested without a control plane.
 */

/** The environment override. An operator may raise or lower it; the default stands otherwise. */
export const GENERATION_TURN_CAP_ENV = 'FORGE_MAX_MODEL_TURNS_PER_GENERATION'

/** MAP's most common production ceiling (68% of respondents cap at <=10 steps). */
export const DEFAULT_MAX_GENERATION_TURNS = 10

/** A cap outside this range is a mistake, not a policy: clamp rather than honour it. */
const MIN_CAP = 1
const MAX_CAP = 100

/**
 * The cap for this run: `FORGE_MAX_MODEL_TURNS_PER_GENERATION`, else the default.
 *
 * A value that is absent, blank or unparseable falls back to the default, and an absurd one
 * is clamped — a mis-typed environment variable must never mean "unlimited", because
 * unlimited is the exact condition the cap exists to prevent.
 */
export function resolveGenerationTurnCap(env: NodeJS.ProcessEnv = process.env): number {
  const raw = env[GENERATION_TURN_CAP_ENV]
  if (raw == null || raw.trim() === '') return DEFAULT_MAX_GENERATION_TURNS
  const parsed = Number.parseInt(raw.trim(), 10)
  if (!Number.isFinite(parsed)) return DEFAULT_MAX_GENERATION_TURNS
  return Math.min(MAX_CAP, Math.max(MIN_CAP, parsed))
}

export type TurnBudgetVerdict =
  | { allowed: true; turnsUsed: number; cap: number }
  | { allowed: false; turnsUsed: number; cap: number; code: 'GENERATION_TURN_CAP'; reason: string }

/**
 * May this generation start another turn?
 *
 * `turnsUsed` is what the generation has ALREADY dispatched (the engine's task-execution
 * ledger), so the check is `>= cap`: at the cap, the next turn is the one that must not
 * happen. Fails CLOSED — a generation at its ceiling is refused, named, and left for a
 * human, rather than being allowed one more attempt to look productive.
 */
export function assessGenerationTurnBudget(input: {
  turnsUsed: number
  cap?: number
}): TurnBudgetVerdict {
  const cap = input.cap ?? DEFAULT_MAX_GENERATION_TURNS
  const turnsUsed = Number.isFinite(input.turnsUsed) ? Math.max(0, input.turnsUsed) : 0
  if (turnsUsed < cap) return { allowed: true, turnsUsed, cap }
  return {
    allowed: false,
    turnsUsed,
    cap,
    code: 'GENERATION_TURN_CAP',
    reason:
      `MODEL TURN CAP: this generation has already dispatched ${turnsUsed} turns (cap ${cap}). ` +
      'Production agents are short, structured and boxed — 68% of them stop at ten model steps ' +
      'and about half at five, while a healthy Forge FEATURE generation costs five. A generation ' +
      'that needs more is not thinking, it is looping, and another turn will not diagnose it. ' +
      'Stop here and read the ENGINE QUEUE: the failing door is earlier than this one. ' +
      `Raise ${GENERATION_TURN_CAP_ENV} only to authorise a longer run deliberately.`,
  }
}

/**
 * One ENGINE-QUEUE line stating how much of the generation budget is spent.
 *
 * Visibility only: it reads a verdict and renders it, so an operator sees how much of the cap
 * is gone BEFORE it fires. The refusal prose is owned by `assessGenerationTurnBudget` and is
 * embedded verbatim — never re-derived here.
 *
 * The ALLOWED line deliberately avoids the words `model turn cap`, `scope`, `held` and `hold`:
 * `readForgeGenerationFacts` (db/forge-run.ts) treats a matching evidence line as a refusal
 * detail, and a healthy run must not be misread as one.
 *
 * Pure by the module's contract: no database, no environment read.
 */
export function renderTurnBudgetLine(verdict: TurnBudgetVerdict): string {
  const remaining = Math.max(0, verdict.cap - verdict.turnsUsed)
  const spent =
    `TURN BUDGET: ${verdict.turnsUsed} of ${verdict.cap} model turns used; ` +
    `${remaining} before the cap`
  if (verdict.allowed) return `${spent}.`
  return `${spent}. ${verdict.reason}`
}
