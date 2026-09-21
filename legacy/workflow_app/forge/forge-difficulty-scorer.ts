// ---------------------------------------------------------------------------
// forge-difficulty-scorer — pre-dispatch difficulty scoring (Forge-native TS
// port of Praxis's transparent hand-weighted logistic).
//
// WHY: Forge needs to predict, BEFORE spending Smith tokens, whether a proposed
// unit of work is likely beyond the worker — then act on it and record it so a
// learned scorer can later replace the hand-set weights without touching call
// sites (Praxis's "DifficultyScorer protocol" / Capability Calibration Loop).
//
// Provenance: the logistic form, feature set, weights and gate thresholds are
// ported from Praxis (Apache-2.0, github.com/adiatmaja/praxis,
// src/orchestrator/core/difficulty.py). Reimplemented here in TypeScript; no
// Praxis code or runtime is imported. Signs are grounded (more files/LOC lower
// success; a machine-checkable acceptance raises it; past worker success is the
// strongest signal); magnitudes are provisional calibration food, not claims.
//
// This is the QUANTITATIVE scorer (measurable / telemetry features). It pairs
// with forge-dispatchability.ts, the QUALITATIVE Lead-judgment vector. Both feed
// the same dispatch gate; this one is what we later learn from run history.
// ---------------------------------------------------------------------------

export type DifficultyFeatures = {
  /** Number of files the leaf touches. */
  filesTouched: number
  /** Estimated LOC delta / the worker's LOC limit (unestimated => worst case 1.0). */
  locRatio: number
  /** This unit's depth in the dependency graph. */
  depDepth: number
  /** Leaf carries a machine-checkable acceptance signal (runnable command). */
  hasAcceptance: boolean
  /** Context tokens needed / the worker's usable context budget (0..1+). */
  contextRatio: number
  /** Observed historical success for this (model, project, shape); else 0.5 neutral. */
  historicalSuccess: number
  /** Repo size bucket: 0 <100 files, 1 100-500, 2 500+. */
  repoSizeBucket: 0 | 1 | 2
  /** True when the unit is a generic/unshaped task rather than a known type. */
  genericType: boolean
}

/** Hand-set log-odds weights, ported from Praxis (Apache-2.0). */
export const DIFFICULTY_WEIGHTS: Readonly<Record<keyof DifficultyFeatures, number>> = {
  filesTouched: -0.45,
  locRatio: -1.1,
  depDepth: -0.35,
  hasAcceptance: 1.3,
  contextRatio: -1.4,
  historicalSuccess: 2.2,
  repoSizeBucket: -0.25,
  genericType: -0.6,
}

export const DIFFICULTY_BIAS = 1.6

/** Neutral prior when a model has no attributable history for this shape. */
export const NEUTRAL_HISTORY = 0.5

/** Gate thresholds on p_success (ported from Praxis). */
export const REJECT_BELOW = 0.35
export const FLAG_BELOW = 0.55

export type DifficultyVerdict = 'reject' | 'flag' | 'dispatch'

/**
 * Predict P(success) for one unit of work on one worker. A seam: the v1
 * implementation is the transparent LogisticScorer below; a LEARNED scorer can
 * implement the same shape later (from run telemetry) and swap in without any
 * call-site change.
 *
 * `logit` is part of the protocol, not an extra: the dispatch ledger records the linear
 * predictor BESIDE the probability so a fit can check that the two agree. A scorer that
 * could not state its own logit would be recording a probability nobody can verify — so a
 * future non-logistic scorer must decide what it writes there rather than inherit the
 * reference logistic's number and call it evidence.
 */
export type DifficultyScorer = {
  score(features: DifficultyFeatures): number
  logit(features: DifficultyFeatures): number
}

export function clampLogit(logit: number): number {
  return Math.max(-60, Math.min(60, logit))
}

export function sigmoid(logit: number): number {
  return 1 / (1 + Math.exp(-clampLogit(logit)))
}

/**
 * THE SCORER'S IDENTITY. A fit is only valid against rows produced by a known scorer, so
 * every recorded prediction carries this. Change it when the weights change: the ledger can
 * then compare vintages instead of silently mixing them.
 */
export const SCORER_ID = 'logistic-hand-weighted-v1'

/**
 * The logit, exposed as a pure function so the RECORD and the PREDICTION cannot drift: the
 * ledger stores the same number the score was computed from, not a recomputation.
 */
export function difficultyLogit(
  features: DifficultyFeatures,
  weights: Readonly<Record<keyof DifficultyFeatures, number>> = DIFFICULTY_WEIGHTS,
  bias: number = DIFFICULTY_BIAS,
): number {
  let logit = bias
  for (const name of Object.keys(weights) as (keyof DifficultyFeatures)[]) {
    logit += weights[name] * (features[name] as number)
  }
  return logit
}

/**
 * Transparent hand-weighted logistic — the v1 placeholder scorer (Praxis logic,
 * TypeScript). score = sigma(bias + w.x), exponent guarded against overflow.
 */
export class LogisticScorer implements DifficultyScorer {
  constructor(
    private readonly weights: Readonly<Record<keyof DifficultyFeatures, number>> = DIFFICULTY_WEIGHTS,
    private readonly bias: number = DIFFICULTY_BIAS,
  ) {}

  score(features: DifficultyFeatures): number {
    // Through `difficultyLogit`, so the number recorded beside a prediction is the number
    // the prediction was made from.
    return sigmoid(this.logit(features))
  }

  /** The linear predictor behind `score`, on the same weights this instance was built with. */
  logit(features: DifficultyFeatures): number {
    return difficultyLogit(features, this.weights, this.bias)
  }
}

/** Classify a p_success into the pre-dispatch gate action. */
export function difficultyGate(pSuccess: number): DifficultyVerdict {
  if (pSuccess < REJECT_BELOW) return 'reject'
  if (pSuccess < FLAG_BELOW) return 'flag'
  return 'dispatch'
}

/** Full pre-dispatch read for one unit of work. */
export function scoreDifficulty(
  scorer: DifficultyScorer,
  features: DifficultyFeatures,
): { pSuccess: number; gate: DifficultyVerdict } {
  const pSuccess = scorer.score(features)
  return { pSuccess, gate: difficultyGate(pSuccess) }
}

