// ---------------------------------------------------------------------------
// forge-dispatchability — deterministic Smith Dispatchability Model.
//
// The gate Forge currently lacks BEFORE Smith: given the Architect contract's
// shape, decide whether one resident Smith can safely consume it in <=3 serial
// chunks, or whether it must go back to Lead (NOT_DISPATCHABLE) rather than let
// Smith become a long token-burning run.
//
// Deterministic, transparent rules FIRST (per the "don't learn too early"
// directive): a human-readable feature vector maps to a verdict + chunk count.
// Later the same contract can be learned from Forge's own run telemetry (widgets
// / elapsed / success / repair / files are already recorded per run), replacing
// the hand-set thresholds with per-model/per-project fit — exactly Praxis's
// intended "Capability Calibration Loop".
//
// Scope rule: a story is executed by ONE resident Smith brain in ONE authoritative
// worktree. We separate WORK UNITS (chunks) without separating UNDERSTANDING
// (the same OpenCode session carries context across chunks).
// ---------------------------------------------------------------------------

/** Hard ceiling on serial chunks a single Smith may take per story. */
export const MAX_CHUNKS_PER_STORY = 3
/** One generation of split; children must never recursively explode. */
export const MAX_SPLIT_GENERATIONS = 1
/** One same-tier repair per chunk before escalation. */
export const MAX_REPAIR_PER_CHUNK = 1
/** One model upgrade step (e.g. Flash -> Pro) before HOLD. */
export const MAX_MODEL_ESCALATION = 1

/** Anti-token-fire budgets — single source of truth for Forge's hard caps. */
export const ANTI_TOKEN_FIRE = {
  MAX_CHUNKS_PER_STORY,
  MAX_SPLIT_GENERATIONS,
  MAX_REPAIR_PER_CHUNK,
  MAX_MODEL_ESCALATION,
} as const

export type DispatchableVerdict = 'SAFE' | 'BOUNDED' | 'HEAVY' | 'NOT_DISPATCHABLE'

/** Each risk dimension measured low(1)..high(5). Deterministic + interpretable. */
export type DispatchabilityFeatures = {
  /** How many independent subsystem responsibilities change (0..n). */
  semanticSurface: number
  /** How many sequential things must become true before the work lands (0..n). */
  dependencyDepth: number
  /** Unresolved implementation decisions still to discover (1..5). */
  uncertainty: number
  /** How much repo state Smith must understand/retain (1..5). */
  contextBurden: number
  /** How many distinct behaviors need independent proof (1..5). */
  proofBurden: number
  /** How much must be correct together (1..5). */
  coupling: number
  /** Known pattern(1) vs new architectural seam(5). */
  changeNovelty: number
  /** Historical worker fit: excellent(1) .. poor(5). */
  workerFit: number
}

export type DispatchabilityResult = {
  verdict: DispatchableVerdict
  /** Serial chunks Smith should take (1..3); 0 when NOT_DISPATCHABLE. */
  chunks: number
  /** Ordered red flags that pushed the verdict up (for Lead to act on). */
  reasons: string[]
}


/** Red-flag tier thresholds on the 1..5 risk dimensions. */
const REDFLAG_LOW = 2 // <= this is "normal" for a SAFE / 1-chunk story
const REDFLAG_MID = 3 // <= this is fine for a BOUNDED / 2-chunk story
const REDFLAG_HIGH = 4 // <= this is fine for a HEAVY / 3-chunk story

function riskDimensionsAbove(features: DispatchabilityFeatures, threshold: number): string[] {
  const out: string[] = []
  if (features.uncertainty > threshold) out.push('uncertainty')
  if (features.contextBurden > threshold) out.push('context-burden')
  if (features.proofBurden > threshold) out.push('proof-burden')
  if (features.coupling > threshold) out.push('coupling')
  if (features.changeNovelty > threshold) out.push('change-novelty')
  if (features.workerFit > threshold) out.push('worker-fit')
  return out
}


/**
 * Classify a Smith assignment by its real shape. Deterministic and transparent.
 * A story is NOT_DISPATCHABLE when it needs more than 3 serial chunks (too deep,
 * too wide) or its risk profile is out of range for any chunking — the fail-fast
 * that sends the story back to Lead instead of into a long Smith burn.
 */
export function dispatchabilityFor(features: DispatchabilityFeatures): DispatchabilityResult {
  const surface = Math.max(0, Math.round(features.semanticSurface))
  const depth = Math.max(0, Math.round(features.dependencyDepth))

  // Cannot be expressed in <=3 coherent chunks -> not dispatchable at all.
  if (depth > MAX_CHUNKS_PER_STORY) {
    return {
      verdict: 'NOT_DISPATCHABLE',
      chunks: 0,
      reasons: [`dependency-depth ${depth} > ${MAX_CHUNKS_PER_STORY} sequential boundaries`],
    }
  }
  if (surface > MAX_CHUNKS_PER_STORY) {
    return {
      verdict: 'NOT_DISPATCHABLE',
      chunks: 0,
      reasons: [`semantic-surface ${surface} > ${MAX_CHUNKS_PER_STORY} independent subsystem responsibilities`],
    }
  }

  // A story needs at most `boundaries` chunks = the larger of how deep the
  // dependency chain is and how many independent surfaces change.
  const naturalChunks = Math.min(MAX_CHUNKS_PER_STORY, Math.max(depth, surface) || 1)

  // Residual risk beyond chunking: any dimension above HIGH is a hard stop.
  const highRisk = riskDimensionsAbove(features, REDFLAG_HIGH)
  if (highRisk.length > 0) {
    return {
      verdict: 'NOT_DISPATCHABLE',
      chunks: 0,
      reasons: [`out-of-range risk: ${highRisk.join(', ')} (worker would be set up to fail)`],
    }
  }

  if (naturalChunks <= 1 && riskDimensionsAbove(features, REDFLAG_LOW).length === 0) {
    return { verdict: 'SAFE', chunks: 1, reasons: [] }
  }
  if (naturalChunks <= 2 && riskDimensionsAbove(features, REDFLAG_MID).length === 0) {
    return { verdict: 'BOUNDED', chunks: 2, reasons: riskDimensionsAbove(features, REDFLAG_LOW) }
  }
  if (naturalChunks <= 3) {
    return { verdict: 'HEAVY', chunks: 3, reasons: riskDimensionsAbove(features, REDFLAG_MID) }
  }
  return { verdict: 'NOT_DISPATCHABLE', chunks: 0, reasons: ['unclassifiable risk profile'] }
}

/** Convenience: does a dispatchability verdict allow a Smith to start at all? */
export function isDispatchable(result: DispatchabilityResult): boolean {
  return result.verdict !== 'NOT_DISPATCHABLE'
}

