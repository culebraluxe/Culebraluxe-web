import { planAssay } from './assay-plan'
import { leadPhaseInstructions, type LeadRunPhase } from './lead-decision'
import { renderModelCostLines } from './model-prices'
import { resolveLane, type LaneDecision, type LaneSession, type SmithGrade } from './lane-policy'
import type { LaneId } from './lanes'
import type { AgentRuntimeRegistry } from './registry'
import type { ForgeTeam } from './team'
import { sessionFromStory, storyPacketInstructions, type StoryPacketFields } from './story-session'
import { buildLeadRoutingDirective } from '../workflow_app/forge/forge-lead-routing-prompt'
import type { RoutingContext } from '../workflow_app/forge/forge-lead-routing'

export type LaneEnqueueInput = {
  lane: LaneId
  story: StoryPacketFields
  session?: Partial<LaneSession>
  smithGrade?: SmithGrade
  extraInstructions?: string | null
  leadPhase?: LeadRunPhase
  /**
   * Trusted routing context for the Lead PRE handoff (Astra handoff). When present
   * on a lead/pre lane, the single `LEAD_ROUTING` directive replaces the legacy
   * multi-marker PRE instructions. Never derived from model output.
   */
  leadRoutingContext?: RoutingContext
  authorizeEmergency?: boolean
  registry?: Pick<AgentRuntimeRegistry, 'hasProfile'>
  team?: ForgeTeam
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
  const extras: string[] = []
  if (input.extraInstructions?.trim()) extras.push(input.extraInstructions.trim())
  if (packet) extras.push(packet)

  if (input.lane === 'lead') {
    const phase = input.leadPhase ?? 'pre'
    // PRE uses the single LEAD_ROUTING directive when trusted context is supplied
    // (the routing validator owns the decision); implement/POST keep their own
    // instructions untouched.
    extras.unshift(
      phase === 'pre' && input.leadRoutingContext
        ? [buildLeadRoutingDirective(input.leadRoutingContext), ...renderModelCostLines()].join('\n')
        : leadPhaseInstructions(phase),
    )
  }

  if (input.lane === 'assay') {
    const plan = planAssay({
      testMode: input.story.testMode,
      assayCommands: input.story.assayCommands,
    })
    if (!plan.ok) {
      return { ok: false, code: plan.code, reason: plan.reason }
    }
    extras.unshift(plan.instructions)
    session.hasAssayPlan = true
  }

  const extra = extras.join('\n\n') || null
  const decision = resolveLane({
    lane: input.lane,
    session,
    smithGrade: input.smithGrade,
    extraInstructions: extra,
    authorizeEmergency: input.authorizeEmergency,
    registry: input.registry,
    team: input.team,
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
