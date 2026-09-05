export type ForgeRoutingBrain = 'reducer' | 'engine'

export function parseForgeRoutingBrain(
  raw: string | null | undefined = process.env.FORGE_ROUTING_BRAIN,
): ForgeRoutingBrain {
  const value = raw?.trim().toLowerCase()
  if (value === 'engine') return 'engine'
  return 'reducer'
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
