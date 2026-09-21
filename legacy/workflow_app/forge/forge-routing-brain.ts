export type ForgeRoutingBrain = 'reducer' | 'engine'

/**
 * ENGINE is the one routing brain (the FORGE_SDLC XML engine is canonical —
 * the driver, gate facts, and every live dogfood run). The legacy reducer is
 * kept only when it is EXPLICITLY selected via FORGE_ROUTING_BRAIN=reducer
 * (test fixture / migration); it is never the default. This makes the single
 * authority explicit instead of a silent fallback.
 */
export function parseForgeRoutingBrain(
  raw: string | null | undefined = process.env.FORGE_ROUTING_BRAIN,
): ForgeRoutingBrain {
  const value = raw?.trim().toLowerCase()
  if (value === 'reducer') return 'reducer'
  return 'engine'
}

export type ForgeDualWriteInput = {
  storyId: string
  reducerTouched: boolean
  engineInstanceActive: boolean
}

export type ForgeDualWriteVerdict =
  | { ok: true }
  | { ok: false; reason: 'dual-write'; storyId: string }

/**
 * Remainder invariant 5: never let the legacy reducer and the engine both
 * write routing for the same story in one visit.
 */
export function detectForgeDualWrite(input: ForgeDualWriteInput): ForgeDualWriteVerdict {
  if (input.reducerTouched && input.engineInstanceActive) {
    return { ok: false, reason: 'dual-write', storyId: input.storyId }
  }
  return { ok: true }
}

export function forgeRoutingBrainShouldFollowReducer(
  brain: ForgeRoutingBrain = parseForgeRoutingBrain(),
): boolean {
  return brain === 'reducer'
}
