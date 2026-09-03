import { resolveLane, type LaneDecision, type LaneSession, type SmithGrade } from './lane-policy'
import type { LaneId } from './lanes'
import type { AgentRuntimeRegistry } from './registry'
import { sessionFromStory, storyPacketInstructions, type StoryPacketFields } from './story-session'

export type LaneEnqueueInput = {
  lane: LaneId
  story: StoryPacketFields
  session?: Partial<LaneSession>
  smithGrade?: SmithGrade
  extraInstructions?: string | null
  authorizeEmergency?: boolean
  registry?: Pick<AgentRuntimeRegistry, 'hasProfile'>
}

export type LaneEnqueueEnvelope = {
  role: string
  modelProfile: string
  specialInstructions: string
  maxAttempts: number
}

export function buildLaneEnqueue(input: LaneEnqueueInput): LaneDecision & {
  envelope?: LaneEnqueueEnvelope
} {
  const session = sessionFromStory(input.story, input.session ?? {})
  const packet = storyPacketInstructions(input.story)
  const extra = [input.extraInstructions?.trim(), packet].filter(Boolean).join('\n\n') || null
  const decision = resolveLane({
    lane: input.lane,
    session,
    smithGrade: input.smithGrade,
    extraInstructions: extra,
    authorizeEmergency: input.authorizeEmergency,
    registry: input.registry,
  })
  if (!decision.ok) return decision
  return {
    ...decision,
    envelope: {
      role: decision.launch.role,
      modelProfile: decision.launch.modelProfile,
      specialInstructions: decision.launch.specialInstructions,
      maxAttempts: 3,
    },
  }
}
