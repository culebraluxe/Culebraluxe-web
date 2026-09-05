import {
  detectForgeDualWrite,
  forgeRoutingBrainShouldFollowReducer,
  parseForgeRoutingBrain,
} from './forge-routing-brain'

export type ForgeNightPlan =
  | { brain: 'reducer'; hydrate: true; follow: true; publish: true; driveEngine: false }
  | { brain: 'engine'; hydrate: false; follow: false; publish: false; driveEngine: true }

export function planForgeNight(
  rawBrain: string | null | undefined = process.env.FORGE_ROUTING_BRAIN,
): ForgeNightPlan {
  const brain = parseForgeRoutingBrain(rawBrain)
  if (brain === 'engine') {
    return { brain, hydrate: false, follow: false, publish: false, driveEngine: true }
  }
  return { brain, hydrate: true, follow: true, publish: true, driveEngine: false }
}

export function refuseForgeNightDualWrite(input: {
  storyId: string
  reducerTouched: boolean
  engineInstanceActive: boolean
}): void {
  const verdict = detectForgeDualWrite(input)
  if (!verdict.ok) {
    throw new Error(`Forge dual-write refused for story ${input.storyId}`)
  }
}

export function forgeNightUsesReducer(
  rawBrain?: string | null,
): boolean {
  return forgeRoutingBrainShouldFollowReducer(parseForgeRoutingBrain(rawBrain))
}
