// ---------------------------------------------------------------------------
// forge-plan-difficulty — bridge from a validated SmithExecutionPlan to a real
// pre-dispatch difficulty read. This is the glue that lets the dispatchability
// gate actually RUN on a Lead-emitted plan: plan -> feature vector -> P(success)
// -> reject/flag/dispatch.
//
// Only plan-derivable features are computed here (files touched = declared
// surfaces; acceptance = a runnable proof command; dependency depth = the serial
// depends_on chain; generic = an unshaped chunk). The telemetry dimensions that
// only real runs reveal (loc_ratio, context_ratio, historical_success,
// repo_size) use neutral defaults until the calibration loop fills them from
// Forge run history.
// ---------------------------------------------------------------------------

import type { SmithExecutionPlan } from '@/legacy/workflow_app/forge/forge-execution-shaping'
import {
  LogisticScorer,
  NEUTRAL_HISTORY,
  type DifficultyFeatures,
  type DifficultyScorer,
  type DifficultyVerdict,
  difficultyGate,
} from '@/legacy/workflow_app/forge/forge-difficulty-scorer'

/**
 * The minimum a plan must expose to be scored. Declared structurally rather than as
 * `SmithExecutionPlan` because the difficulty features read ONLY these fields (surface,
 * proof, id, dependsOn) — so a plan read back from the database scores identically to the
 * same plan in flight, which is what lets the ledger record the prediction the gate acted
 * on rather than a lookalike.
 */
export type PlanShapeForDifficulty = {
  chunks: Array<{
    id: number
    surface?: string[] | null
    proof?: string | null
    dependsOn?: number[] | null
  }>
}

/** A proof is "runnable" when it names a real command token or a backtick shell
 * command — the machine-checkable acceptance signal (mirrors Praxis's
 * _RUNNABLE_SIGNAL). Prose alone does not count. */
const RUNNABLE_SIGNAL = /\b(pnpm|npm|node|tsx|vitest|jest|tsc|git|yarn|npx|pnpm exec)\b|`[^`]+`/i

export function chunkHasRunnableProof(proof: string | null | undefined): boolean {
  return Boolean(proof && RUNNABLE_SIGNAL.test(proof))
}

function chunkDepth(
  chunk: { id: number; dependsOn?: number[] | null },
  byId: Map<number, { id: number; dependsOn?: number[] | null }>,
): number {
  const deps = (chunk.dependsOn ?? []).filter((d) => d < chunk.id)
  if (deps.length === 0) return 1
  return 1 + Math.max(...deps.map((d) => (byId.get(d) ? chunkDepth(byId.get(d)!, byId) : 0)))
}

/** Derive the plan-visible difficulty features from a validated execution plan.
 * Telemetry dimensions (loc_ratio, context_ratio, historical_success,
 * repo_size_bucket) use neutral defaults until the calibration loop fills them
 * from real run history. */
export function difficultyFeaturesForPlan(plan: PlanShapeForDifficulty): DifficultyFeatures {
  const chunks = plan.chunks
  const surfaces = new Set(chunks.flatMap((c) => c.surface ?? []))
  const byId = new Map(chunks.map((c) => [c.id, c]))
  const maxDepth = chunks.length > 0 ? Math.max(...chunks.map((c) => chunkDepth(c, byId))) : 0
  const filesTouched = surfaces.size
  return {
    filesTouched,
    locRatio: 0.3, // telemetry default
    depDepth: maxDepth,
    hasAcceptance: chunks.some((c) => chunkHasRunnableProof(c.proof)),
    contextRatio: 0.3, // telemetry default
    historicalSuccess: NEUTRAL_HISTORY,
    repoSizeBucket: 1, // telemetry default
    genericType: chunks.some((c) => !Array.isArray(c.surface) || c.surface.length === 0 || !chunkHasRunnableProof(c.proof)),
  }
}

/**
 * The four features that are TELEMETRY DEFAULTS rather than measurements — the ones
 * `forge-plan-difficulty.ts` documents as waiting for the calibration loop. Recorded
 * explicitly so a fit never mistakes a placeholder for evidence.
 */
export const TELEMETRY_DEFAULT_FEATURES: ReadonlyArray<keyof DifficultyFeatures> = [
  'locRatio',
  'contextRatio',
  'historicalSuccess',
  'repoSizeBucket',
]

/**
 * The plan-visible features TOGETHER WITH which of them were actually measured.
 *
 * The split matters more than the numbers: four of the eight are constants for now, so a
 * prediction made today rests on four measurements and four defaults. A row that recorded
 * only the vector would let a future fit learn from the constants as though they were data.
 */
export function difficultyFeaturesForPlanDetailed(plan: PlanShapeForDifficulty): {
  features: DifficultyFeatures
  measured: Array<keyof DifficultyFeatures>
} {
  const all = difficultyFeaturesForPlan(plan)
  const measured = (Object.keys(all) as Array<keyof DifficultyFeatures>).filter(
    (name) => !TELEMETRY_DEFAULT_FEATURES.includes(name),
  )
  return { features: all, measured }
}

/**
 * The same read as `scorePlan`, plus everything the LEDGER needs: which features were
 * measured and the exact logit the probability came from. One function so a recorded
 * prediction can never disagree with the prediction that was acted on.
 */
export function scorePlanDetailed(
  plan: PlanShapeForDifficulty,
  scorer: DifficultyScorer = new LogisticScorer(),
): {
  features: DifficultyFeatures
  measured: Array<keyof DifficultyFeatures>
  logit: number
  pSuccess: number
  gate: DifficultyVerdict
} {
  const { features, measured } = difficultyFeaturesForPlanDetailed(plan)
  // FROM THE SCORER ITSELF. Taking the logit from the default weights while the score came
  // from a passed scorer would record a prediction nobody made — the drift the ledger
  // exists to prevent, caught by this module's own test.
  const logit = scorer.logit(features)
  const pSuccess = scorer.score(features)
  return { features, measured, logit, pSuccess, gate: difficultyGate(pSuccess) }
}

/** Full pre-dispatch read for a Lead-emitted plan: features + P(success) + gate. */
export function scorePlan(
  plan: PlanShapeForDifficulty,
  scorer: DifficultyScorer = new LogisticScorer(),
): { features: DifficultyFeatures; pSuccess: number; gate: DifficultyVerdict } {
  const features = difficultyFeaturesForPlan(plan)
  const pSuccess = scorer.score(features)
  return { features, pSuccess, gate: difficultyGate(pSuccess) }
}

