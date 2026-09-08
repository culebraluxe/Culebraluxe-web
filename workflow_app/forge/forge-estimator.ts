// ---------------------------------------------------------------------------
// SWAG estimator — turn a fuzzy scope into a bounded forecast (tokens, cost,
// time, SLOC, risk) so the harness can set a real budget and learn from
// actuals. Pure + DB-free. The "art": complexity x toolkit x seams x acceptance.
// The "science": unit rates that get recalibrated from real story-run actuals.
//
// Rates are coarse, operator-set defaults (like model prices) — NOT vendor
// quotes. Calibrate from actuals over time.
// ---------------------------------------------------------------------------

export type WorkComplexity = 'low' | 'medium' | 'high'
export type WorkToolkit = 'greenfield' | 'brownfield'
export type ModelGrade = 'flash' | 'pro'
export type WorkRisk = 'low' | 'medium' | 'high'

export type WorkEstimateFactors = {
  workstream: string
  complexity: WorkComplexity
  toolkit: WorkToolkit
  modelGrade: ModelGrade
  /** Areas/files in scope — feed straight from SHAPE unit seams. */
  seamsCount: number
  acceptanceCount: number
}

export type UnitRates = {
  tokensPerPoint: number
  costPerPoint: number
  minutesPerPoint: number
  slocPerPoint: number
}

/** Default per-grade rates. Pro costs far more per point but is more effective
 * per point (fewer minutes) — the tradeoff the estimator should expose. */
export const DEFAULT_UNIT_RATES: Record<ModelGrade, UnitRates> = {
  flash: { tokensPerPoint: 1200, costPerPoint: 0.02, minutesPerPoint: 3, slocPerPoint: 22 },
  pro: { tokensPerPoint: 1100, costPerPoint: 0.25, minutesPerPoint: 1.2, slocPerPoint: 18 },
}

const COMPLEXITY_SCORE: Record<WorkComplexity, number> = { low: 1, medium: 3, high: 8 }

export function complexityScore(complexity: WorkComplexity): number {
  return COMPLEXITY_SCORE[complexity]
}

/** Normalized work points: complexity x toolkit x seam fan-out x acceptance. */
export function workPoints(f: WorkEstimateFactors): number {
  const toolkitMult = f.toolkit === 'greenfield' ? 1.5 : 1.0
  const seamMult = 0.75 + 0.25 * Math.max(0, f.seamsCount)
  const acceptMult = 1 + 0.15 * Math.max(0, f.acceptanceCount)
  return Math.round(complexityScore(f.complexity) * toolkitMult * seamMult * acceptMult * 10) / 10
}

export function estimatedRisk(f: WorkEstimateFactors): WorkRisk {
  if (f.complexity === 'high') return 'high'
  if (f.toolkit === 'greenfield' && f.complexity === 'medium') return 'high'
  if (f.seamsCount > 8 || f.acceptanceCount > 12) return 'high'
  if (f.complexity === 'medium' || f.toolkit === 'greenfield') return 'medium'
  return 'low'
}

export type WorkForecast = {
  points: number
  estimatedTokens: number
  estimatedCostUsd: number
  estimatedMinutes: number
  estimatedSloc: number
  risk: WorkRisk
}

export function estimateWork(
  f: WorkEstimateFactors,
  rates?: UnitRates,
): WorkForecast {
  const r = rates ?? DEFAULT_UNIT_RATES[f.modelGrade]
  const points = workPoints(f)
  return {
    points,
    estimatedTokens: Math.round(points * r.tokensPerPoint),
    estimatedCostUsd: Math.round(points * r.costPerPoint * 100) / 100,
    estimatedMinutes: Math.round(points * r.minutesPerPoint),
    estimatedSloc: Math.round(points * r.slocPerPoint),
    risk: estimatedRisk(f),
  }
}

export type WorkActual = { points: number; tokens: number; costUsd: number; minutes: number }

/** Recalibrate unit rates from real actuals (the "5 years / 2 promotions" bit,
 * made measurable). Empty input returns the defaults. */
export function ratesFromActuals(actuals: WorkActual[], fallback?: UnitRates): UnitRates {
  if (actuals.length === 0) {
    return fallback ?? DEFAULT_UNIT_RATES.flash
  }
  const total = actuals.reduce(
    (a, b) => ({ points: a.points + b.points, tokens: a.tokens + b.tokens, costUsd: a.costUsd + b.costUsd, minutes: a.minutes + b.minutes }),
    { points: 0, tokens: 0, costUsd: 0, minutes: 0 },
  )
  const pts = Math.max(total.points, 1)
  return {
    tokensPerPoint: total.tokens / pts,
    costPerPoint: total.costUsd / pts,
    minutesPerPoint: total.minutes / pts,
    slocPerPoint: (fallback ?? DEFAULT_UNIT_RATES.flash).slocPerPoint,
  }
}

// -- Cost widgets: stable, model-relative cost units (NOT real money). --------
// The vendor invoices a day late; we instead record widgets now so calibration
// has fuel: widgets = model_weight x elapsed minutes.

/** Relative weight for a model id. Deterministic Assay is free. Unknown -> null. */
export function modelWidgetWeight(model: string | null | undefined): number | null {
  const key = (model ?? '').trim().toLowerCase()
  if (!key) return null
  if (key.includes('deterministic-assay')) return 0
  if (key.includes('flash')) return 1
  if (key.includes('pro') || key.includes('reasoner')) return 10
  if (key.includes('chat')) return 4
  return null
}

/** Widgets burned by one run = model weight x elapsed minutes. Unknown model -> null. */
export function costWidgets(model: string | null | undefined, elapsedMinutes: number): number | null {
  const weight = modelWidgetWeight(model)
  if (weight === null || !Number.isFinite(elapsedMinutes) || elapsedMinutes < 0) return null
  return Math.round(weight * elapsedMinutes * 100) / 100
}
