import { planAssay } from './assay-plan'
import { leadPhaseInstructions, type LeadRunPhase } from './lead-decision'
import { resolveLane, type LaneDecision, type LaneSession, type SmithGrade } from './lane-policy'
import type { LaneId } from './lanes'
import { qaGateInstructions } from './qa-decision'
import type { AgentRuntimeRegistry } from './registry'
import type { ForgeTeam } from './team'
import { sessionFromStory, storyPacketInstructions, type StoryPacketFields } from './story-session'

export type LaneEnqueueInput = {
  lane: LaneId
  story: StoryPacketFields
  session?: Partial<LaneSession>
  smithGrade?: SmithGrade
  extraInstructions?: string | null
  leadPhase?: LeadRunPhase
  authorizeEmergency?: boolean
  registry?: Pick<AgentRuntimeRegistry, 'hasProfile'>
  team?: ForgeTeam
  candidateShas?: string[]
  parallelGroupId?: string | null
  parallelSlot?: number | null
  parallelSize?: number | null
  splitAssignment?: string | null
}

export type LaneEnqueueEnvelope = {
  role: string
  lane: LaneId
  runPhase: LeadRunPhase | null
  modelProfile: string
  playerId: string
  providerId: string
  modelId: string
  harnessId: string
  fieldId: string
  specialInstructions: string
  maxAttempts: number
  candidateShas: string[]
  parallelGroupId: string | null
  parallelSlot: number | null
  parallelSize: number | null
  splitAssignment: string | null
}

export function buildLaneEnqueue(input: LaneEnqueueInput): LaneDecision & {
  envelope?: LaneEnqueueEnvelope
} {
  const session = sessionFromStory(input.story, input.session ?? {})
  const packet = storyPacketInstructions(input.story)
  const candidates = (input.candidateShas ?? []).map((sha) => sha.trim()).filter(Boolean)
  const extras: string[] = []
  if (input.extraInstructions?.trim()) extras.push(input.extraInstructions.trim())
  if (packet) extras.push(packet)

  if (input.lane === 'lead') {
    const phase = input.leadPhase ?? 'pre'
    extras.unshift(leadPhaseInstructions(phase))
    if (phase === 'post' && candidates.length > 0) {
      extras.unshift(
        `Typed Forge candidate set to integrate: ${candidates.join(', ')}. The worktree starts from the first candidate; integrate every remaining candidate before QA.`,
      )
    }
  }

  if (input.lane === 'inspector') {
    extras.unshift(qaGateInstructions(candidates))
    session.hasInlineDiff = candidates.length > 0 || session.hasInlineDiff
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

  if (input.lane === 'smith' && input.splitAssignment?.trim()) {
    extras.unshift(
      `Lead bounded split assignment ${input.parallelSlot ?? '?'}/${input.parallelSize ?? '?'}: ${input.splitAssignment.trim()}`,
    )
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
      lane: decision.launch.lane,
      runPhase: input.lane === 'lead' ? input.leadPhase ?? 'pre' : null,
      modelProfile: decision.launch.modelProfile,
      playerId: decision.launch.playerId,
      providerId: decision.launch.providerId,
      modelId: decision.launch.modelId,
      harnessId: decision.launch.harnessId,
      fieldId: decision.launch.fieldId,
      specialInstructions: decision.launch.specialInstructions,
      maxAttempts: 3,
      candidateShas: candidates,
      parallelGroupId: input.parallelGroupId ?? null,
      parallelSlot: input.parallelSlot ?? null,
      parallelSize: input.parallelSize ?? null,
      splitAssignment: input.splitAssignment?.trim() || null,
    },
  }
}
