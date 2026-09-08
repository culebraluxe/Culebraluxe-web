// ---------------------------------------------------------------------------
// V2 — Ripwire surface multiplier.
//
// The base estimator uses a WEAK prior (self-reported complexity -> points).
// Ripwire gives a MEASURED forward signal of the real change surface before we
// commit: files, symbols, cognitive complexity (ccx), blast radius (callers),
// and test obligations. This module turns that pack into a multiplier on the
// prior forecast, so a "medium" guess becomes a measured scope.
//
// Pure + DB-free. Coefficients are operator-tuned constants (like model prices)
// and should be re-fit by the V3 calibration layer once real actuals exist.
// ---------------------------------------------------------------------------

export type RipwireSurface = {
  files: number
  ccxTotal: number
  callers: number
  symbols: number
}

export const SURFACE_RATES = {
  /** multiplier contribution per distinct file in scope. */
  filesWeight: 0.06,
  /** multiplier contribution per unit of total cognitive complexity. */
  ccxWeight: 0.003,
  /** multiplier contribution per one-hop caller (blast radius). */
  callersWeight: 0.02,
  min: 0.6,
  max: 3.5,
}

/** Extract the measured surface from a ripwire pack's text. Empty -> zero. */
export function ripwireSurfaceFromPack(pack: string | null | undefined): RipwireSurface {
  if (!pack) return { files: 0, ccxTotal: 0, callers: 0, symbols: 0 }
  const files = new Set<string>()
  const fileRe = /p="([^"]+)"/g
  let m: RegExpExecArray | null
  while ((m = fileRe.exec(pack))) {
    const path = m[1].trim()
    if (path && !path.includes('_')) files.add(path)
  }
  let ccxTotal = 0
  let symbols = 0
  const ccxRe = /ccx="(\d+)"/g
  while ((m = ccxRe.exec(pack))) {
    ccxTotal += Number(m[1]) || 0
    symbols += 1
  }
  let callers = 0
  const callerRe = /rel="caller"/g
  while (callerRe.exec(pack)) callers += 1

  return { files: files.size, ccxTotal, callers, symbols }
}

/** Map a measured surface to an estimate multiplier (>= min, <= max). 1 = no change. */
export function surfaceMultiplier(surface: RipwireSurface): number {
  if (surface.files === 0 && surface.ccxTotal === 0 && surface.callers === 0) {
    return 1 // no surface evidence -> unchanged prior (identity, like V3)
  }
  const raw =
    surface.files * SURFACE_RATES.filesWeight +
    surface.ccxTotal * SURFACE_RATES.ccxWeight +
    surface.callers * SURFACE_RATES.callersWeight
  const clamped = Math.max(SURFACE_RATES.min, Math.min(SURFACE_RATES.max, raw))
  return Math.round(clamped * 100) / 100
}

/** Adjust a prior forecast by the ripwire surface multiplier. */
export function applySurfaceMultiplier<T extends { estimatedTokens: number; estimatedCostUsd: number; estimatedMinutes: number; estimatedSloc: number }>(
  forecast: T,
  multiplier: number,
): T {
  return {
    ...forecast,
    estimatedTokens: Math.round(forecast.estimatedTokens * multiplier),
    estimatedCostUsd: Math.round(forecast.estimatedCostUsd * multiplier * 100) / 100,
    estimatedMinutes: Math.round(forecast.estimatedMinutes * multiplier),
    estimatedSloc: Math.round(forecast.estimatedSloc * multiplier),
  }
}
