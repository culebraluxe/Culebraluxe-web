// ---------------------------------------------------------------------------
// ENG — V3 STATS / CONFIDENCE LAYER (STUB — NOT ACTIVE).
//
// Purpose (captured 2026-09-07, from the operator's Harvard Stats-for-Engineers
// instinct): once a labeled time series of real runs accumulates, turn the
// estimator from a point forecast into a calibrated prediction with a
// confidence interval and a goodness-of-fit read.
//
//   estimator today      = weak prior: base factors -> points -> forecast
//   + ripwire (V2?)      = measured forward surface (files, ccx, blast, tests)
//   + THIS (V3)          = learn from ACTUALS and report "estimate +/- band",
//                          R^2, and residual bias — the regression layer.
//
// Regression inputs (features)     = base factors (complexity/toolkit/seams/
//                                    acceptance) AND ripwire surface score.
// Outcome (label)                 = actual widgets / minutes / SLOC recorded on
//                                    storyboard_story_run once telemetry lands
//                                    (model_used/cost currently not persisted —
//                                    see the estimator telemetry fix).
//
// Why it is OFF: with ~zero labeled actuals today, fitting is meaningless (R^2
// over n<... samples is noise). Everything below is an interface + documented
// intent. When real rows accumulate, fill the bodies and gate activation on
// MIN_SAMPLES. Nothing in this module runs until then.
// ---------------------------------------------------------------------------

/** One labeled story outcome once telemetry is wired. */
export type CalibrationSample = {
  storyId: string
  /** Feature vector at estimate time. */
  features: {
    /** Estimated work points (base prior). */
    points: number
    /** Ripwire surface score (files x ccx x blast x test obligations), when available. */
    surfaceScore: number | null
    seamsCount: number
    complexityTier: 'low' | 'medium' | 'high'
    toolkit: 'greenfield' | 'brownfield'
    modelGrade: 'flash' | 'pro'
  }
  /** Measured outcome from the finished run (widgets / minutes / real SLOC). */
  actual: {
    costWidgets: number
    minutes: number
    sloc: number
  }
  finishedAt: string
}

/** Learned model: per-segment weights + fit diagnostics (V3). */
export type CalibratedModel = {
  segments: Array<{
    key: string
    sampleCount: number
    /** weight on prior points (shrinks toward 1 until enough data). */
    pointsWeight: number
    /** weight on ripwire surface (grows as measured signal proves out). */
    surfaceWeight: number
  }>
  /** Pooled goodness of fit. null until a meaningful sample count exists. */
  rSquared: number | null
  residualBias: number | null
  sampleCount: number
}

/** A forecast plus its confidence band and how much to trust it. */
export type EstimateWithInterval = {
  estimate: number
  lower: number
  upper: number
  /** How wide/tight the band is; grows honest at small n. */
  confidence: 'exploratory' | 'moderate' | 'calibrated'
  sampleCount: number
  note: string
}

/**
 * Minimum labeled samples before any fit is reported. Below this, return null
 * and let the point estimate + declared-wide prior stand.
 */
export const MIN_FIT_SAMPLES = 10

/**
 * V3 — fit a calibrated model from real actuals.
 *
 * TODO (turn on once actuals accumulate via the telemetry fix):
 *   - filter samples to recent N (rolling window) so the model tracks how
 *     current tooling/team behaves instead of all history;
 *   - optionally segment by (complexity tier x modelGrade) so flash vs pro and
 *     easy vs hard each learn their own slope;
 *   - regress actual (widgets/minutes/SLOC) on [prior points, ripwire surface];
 *     shrink prior->learned weights toward the prior until samples are adequate;
 *   - compute R^2 / adjusted R^2 and residual bias (are we systematically over-
 *     or under-estimating? adjust the intercept).
 * Returns null until >= MIN_FIT_SAMPLES and telemetry is verified.
 */
export function fitCalibratedModel(
  samples: CalibrationSample[],
): CalibratedModel | null {
  if (samples.length < MIN_FIT_SAMPLES) return null
  // V3: no real actuals yet — see module header. Body intentionally empty.
  return null
}

/**
 * V3 — produce an estimate with a confidence interval from a fitted model.
 *
 * TODO (turn on once fitCalibratedModel returns a model):
 *   - combine prior points x surface weight -> point estimate;
 *   - widen the band with a t/prediction interval at small n, narrow as
 *     sampleCount grows (empirical residuals or t-approximation);
 *   - surface residual bias to correct systematic over/under-estimation.
 * Returns null until a calibrated model exists.
 */
export function predictWithInterval(
  _model: CalibratedModel,
  _features: CalibrationSample['features'],
): EstimateWithInterval | null {
  // V3: not active. See module header.
  return null
}
