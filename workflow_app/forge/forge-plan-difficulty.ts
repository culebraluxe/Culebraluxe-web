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

import type { SmithExecutionPlan } from './forge-execution-shaping'
import {
  LogisticScorer,
  NEUTRAL_HISTORY,
  type DifficultyFeatures,
  type DifficultyScorer,
  type DifficultyVerdict,
  difficultyGate,
} from './forge-difficulty-scorer'

/** A proof is "runnable" when it names a real command token or a backtick shell
 * command — the machine-checkable acceptance signal (mirrors Praxis's
 * _RUNNABLE_SIGNAL). Prose alone does not count. */
const RUNNABLE_SIGNAL = /\b(pnpm|npm|node|tsx|vitest|jest|tsc|git|yarn|npx|pnpm exec)\b|`[^`]+`/i

export function chunkHasRunnableProof(proof: string | null | undefined): boolean {
  return Boolean(proof && RUNNABLE_SIGNAL.test(proof))
}

function chunkDepth(chunk: { id: number; dependsOn?: number[] }, byId: Map<number, { id: number; dependsOn?: number[] }>): number {
  const deps = (chunk.dependsOn ?? []).filter((d) => d < chunk.id)
  if (deps.length === 0) return 1
  return 1 + Math.max(...deps.map((d) => (byId.get(d) ? chunkDepth(byId.get(d)!, byId) : 0)))
}

/** Derive the plan-visible difficulty features from a validated execution plan.
 * Telemetry dimensions (loc_ratio, context_ratio, historical_success,
 * repo_size_bucket) use neutral defaults until the calibration loop fills them
 * from real run history. */
export function difficultyFeaturesForPlan(plan: SmithExecutionPlan): DifficultyFeatures {
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

/** Full pre-dispatch read for a Lead-emitted plan: features + P(success) + gate. */
export function scorePlan(
  plan: SmithExecutionPlan,
  scorer: DifficultyScorer = new LogisticScorer(),
): { features: DifficultyFeatures; pSuccess: number; gate: DifficultyVerdict } {
  const features = difficultyFeaturesForPlan(plan)
  const pSuccess = scorer.score(features)
  return { features, pSuccess, gate: difficultyGate(pSuccess) }
}

