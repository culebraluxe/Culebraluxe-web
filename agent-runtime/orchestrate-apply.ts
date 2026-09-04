import { buildLaneEnqueue, type LaneEnqueueEnvelope } from './enqueue-lane'
import {
  executionContractFailureText,
  validateExecutionContract,
  type ExecutionContractResult,
} from './execution-contract'
import { createAgentRuntimeRegistry } from './factory'
import { pickLane, storyFieldsFromBoardAndGit } from './orchestrate'
import type { AgentRuntimeRegistry } from './registry'
import type { LaneId } from './lanes'
import type { StoryPacketFields } from './story-session'
import { FORGE_FIELDS, configuredForgeTeam, type ForgeTeam } from './team'
import { isAssayTerminalRole, isCleanAssayEvidence } from './candidate-assay-handoff'
import { withAssayCandidateDirective } from './assay-evidence'
import { decideForgeTransition, type ForgeTransitionDecision } from './forge-transition'
import type { LeadDecisionCode, LeadRunPhase } from './lead-decision'
import type { EnqueueForgeAgentWorkInput } from '../db/agent-work-v61'

export type BareWorkItem = {
  id: string
  storyId: string
  state: string
  role: string | null
  lane?: LaneId | null
  modelProfile: string | null
  executionEnvironment: string | null
  executionPolicy: string
  priority: number
}

export type HydrateDeps = {
  listItems: () => Promise<BareWorkItem[] | null>
  getStory: (id: string) => Promise<StoryPacketFields | null>
  enqueue: (input: EnqueueForgeAgentWorkInput) => Promise<unknown>
  repoRoot?: string
  registry?: AgentRuntimeRegistry
  team?: ForgeTeam
}

function gateSmithEnvelope(input: {
  lane: LaneId
  story: StoryPacketFields
  executionTarget: string | null | undefined
  envelope: LaneEnqueueEnvelope
  registry?: AgentRuntimeRegistry
}): ExecutionContractResult | null {
  if (input.lane !== 'smith') return null
  const field = FORGE_FIELDS[input.envelope.fieldId as keyof typeof FORGE_FIELDS]
  return validateExecutionContract({
    story: input.story,
    executionTarget: input.executionTarget,
    modelProfile: input.envelope.modelProfile,
    registry: input.registry ?? createAgentRuntimeRegistry(),
    field: { id: input.envelope.fieldId, ready: Boolean(field?.ready) },
  })
}

function enqueueInput(
  storyId: string,
  envelope: LaneEnqueueEnvelope,
  options: {
    priority?: number
    executionPolicy?: string
    executionEnvironment?: string | null
  } = {},
): EnqueueForgeAgentWorkInput {
  return {
    storyId,
    role: envelope.role,
    lane: envelope.lane,
    runPhase: envelope.runPhase,
    modelProfile: envelope.modelProfile,
    playerId: envelope.playerId,
    providerId: envelope.providerId,
    modelId: envelope.modelId,
    harnessId: envelope.harnessId,
    fieldId: envelope.fieldId,
    specialInstructions: envelope.specialInstructions,
    candidateShas: envelope.candidateShas,
    priority: options.priority,
    maxAttempts: envelope.maxAttempts,
    executionPolicy: options.executionPolicy ?? 'Unattended OK',
    executionEnvironment: options.executionEnvironment ?? 'DEV',
    parallelGroupId: envelope.parallelGroupId,
    parallelSlot: envelope.parallelSlot,
    parallelSize: envelope.parallelSize,
    splitAssignment: envelope.splitAssignment,
  }
}

export function isCleanAssayResult(input: {
  resultStatus?: string | null
  testsSummary?: string | null
}): boolean {
  return isCleanAssayEvidence(input)
}

export function assayFailureEvidence(input: {
  testsSummary?: string | null
  failedCommands?: string[] | null
  assayCommands?: string[] | null
}): string | null {
  const summary = (input.testsSummary ?? '').trim()
  const commands = (input.assayCommands ?? input.failedCommands ?? [])
    .map((command) => command.trim())
    .filter(Boolean)
  if (commands.length === 0) return summary || null
  const commandEvidence = `assay commands: ${commands.join(', ')}`
  return summary ? `${summary} | ${commandEvidence}` : commandEvidence
}

export async function hydrateBareReadyItems(deps: HydrateDeps): Promise<string[]> {
  const items = (await deps.listItems()) ?? []
  const bare = items.filter(
    (item) => item.state === 'Ready' && (!item.role || !item.modelProfile || !item.lane),
  )
  const stamped: string[] = []
  const team = deps.team ?? configuredForgeTeam()
  const registry = deps.registry ?? createAgentRuntimeRegistry(undefined, undefined, team)

  for (const item of bare) {
    const story = await deps.getStory(item.storyId)
    if (!story) continue
    const merged = storyFieldsFromBoardAndGit(story, item.storyId, deps.repoRoot)
    const lane = pickLane({ story: merged })
    const decision = buildLaneEnqueue({
      lane,
      story: merged,
      registry,
      team,
      ...(lane === 'lead' ? { leadPhase: 'pre' as const } : {}),
    })
    if (!decision.ok || !decision.envelope) {
      console.log('hydrate skip', item.storyId, decision.ok ? 'no envelope' : decision.code)
      continue
    }

    const contract = gateSmithEnvelope({
      lane,
      story: merged,
      executionTarget: item.executionEnvironment ?? 'DEV',
      envelope: decision.envelope,
      registry,
    })
    if (contract && !contract.ok) {
      console.log(
        'hydrate skip', item.storyId, 'smith', 'execution-contract',
        executionContractFailureText(contract) ?? contract.code,
      )
      continue
    }

    await deps.enqueue(enqueueInput(item.storyId, decision.envelope, {
      priority: item.priority,
      executionPolicy: item.executionPolicy || 'Unattended OK',
      executionEnvironment: item.executionEnvironment ?? 'DEV',
    }))
    stamped.push(`${item.storyId}:${lane}`)
  }
  return stamped
}

function transitionForFinishedLane(input: {
  finishedRole: string
  candidateShas: string[]
  splitComplete?: boolean
  leadPhase?: LeadRunPhase | null
  leadDecision?: LeadDecisionCode | null
  leadSplitCount?: number | null
  leadReason?: string | null
  qaApproved?: boolean | null
  qaReason?: string | null
}): ForgeTransitionDecision | null {
  const firstCandidate = input.candidateShas[0] ?? null
  if (input.finishedRole === 'scout') return decideForgeTransition({ type: 'scout-complete' })
  if (input.finishedRole === 'architect') return decideForgeTransition({ type: 'architect-complete' })
  if (input.finishedRole === 'builder') {
    return input.splitComplete
      ? decideForgeTransition({ type: 'smith-split-complete', candidateShas: input.candidateShas })
      : decideForgeTransition({ type: 'smith-complete', candidateSha: firstCandidate })
  }
  if (input.finishedRole === 'reviewer') {
    return input.qaApproved
      ? decideForgeTransition({ type: 'qa-pass', candidateSha: firstCandidate })
      : decideForgeTransition({
          type: 'qa-fail',
          code: 'QA_REVIEW_FAILED',
          detail: input.qaReason ?? 'Independent QA rejected the candidate.',
        })
  }
  if (input.finishedRole === 'lead') {
    if (input.leadPhase === 'pre') {
      return decideForgeTransition({
        type: 'lead-pre',
        decision: input.leadDecision ?? null,
        splitCount: input.leadSplitCount ?? null,
        detail: input.leadReason ?? null,
      })
    }
    if (input.leadPhase === 'implement') {
      return decideForgeTransition({ type: 'lead-implement-complete', candidateSha: firstCandidate })
    }
    if (input.leadPhase === 'post') {
      return decideForgeTransition({
        type: 'lead-post',
        decision: input.leadDecision ?? null,
        candidateSha: firstCandidate,
        detail: input.leadReason ?? null,
      })
    }
  }
  return null
}

export async function followFinishedLane(input: {
  storyId: string
  finishedRole: string
  resultStatus?: string | null
  testsSummary?: string | null
  getStory: (id: string) => Promise<StoryPacketFields | null>
  enqueue: HydrateDeps['enqueue']
  repoRoot?: string
  registry?: AgentRuntimeRegistry
  team?: ForgeTeam
  candidateSha?: string | null
  candidateShas?: string[]
  splitComplete?: boolean
  parallelGroupId?: string | null
  leadPhase?: LeadRunPhase | null
  leadDecision?: LeadDecisionCode | null
  leadSplitCount?: number | null
  leadAssignments?: string[]
  leadReason?: string | null
  qaApproved?: boolean | null
  qaReason?: string | null
}): Promise<string | null> {
  if (isAssayTerminalRole(input.finishedRole)) return null

  const ok = !input.resultStatus || /complete|success|pass/i.test(input.resultStatus)
  if (!ok) return null

  const story = await input.getStory(input.storyId)
  if (!story) return null
  const merged = storyFieldsFromBoardAndGit(story, input.storyId, input.repoRoot)
  const candidateShas = [
    ...(input.candidateShas ?? []),
    ...(input.candidateSha ? [input.candidateSha] : []),
  ].map((sha) => sha.trim()).filter((sha, index, all) => Boolean(sha) && all.indexOf(sha) === index)

  const transition = transitionForFinishedLane({
    finishedRole: input.finishedRole,
    candidateShas,
    splitComplete: input.splitComplete,
    leadPhase: input.leadPhase,
    leadDecision: input.leadDecision,
    leadSplitCount: input.leadSplitCount,
    leadReason: input.leadReason,
    qaApproved: input.qaApproved,
    qaReason: input.qaReason,
  })
  if (!transition || !transition.nextLane || transition.action === 'hold-human') return null

  const team = input.team ?? configuredForgeTeam()
  const registry = input.registry ?? createAgentRuntimeRegistry(undefined, undefined, team)
  const lane = transition.nextLane
  const leadPhase = transition.nextPhase ?? undefined

  if (transition.action === 'enqueue-smith-split') {
    const count = transition.parallelCount ?? 0
    const assignments = input.leadAssignments ?? []
    const groupId = input.parallelGroupId?.trim() || null
    if (!groupId || assignments.length !== count || (count !== 2 && count !== 3)) return null

    for (let slot = 1; slot <= count; slot += 1) {
      const decision = buildLaneEnqueue({
        lane: 'smith',
        story: merged,
        registry,
        team,
        parallelGroupId: groupId,
        parallelSlot: slot,
        parallelSize: count,
        splitAssignment: assignments[slot - 1],
      })
      if (!decision.ok || !decision.envelope) return null
      const contract = gateSmithEnvelope({
        lane: 'smith', story: merged, executionTarget: 'DEV', envelope: decision.envelope, registry,
      })
      if (contract && !contract.ok) return null
      await input.enqueue(enqueueInput(input.storyId, decision.envelope))
    }
    return `smith:split:${count}`
  }

  const extraInstructions = lane === 'assay' && candidateShas[0]
    ? withAssayCandidateDirective(
        `Forge V6.1 exact-candidate Assay compatibility directive. Typed candidate_shas is authoritative: ${candidateShas[0]}.`,
        candidateShas[0],
      )
    : null

  const decision = buildLaneEnqueue({
    lane,
    story: merged,
    registry,
    team,
    candidateShas,
    ...(leadPhase ? { leadPhase } : {}),
    ...(extraInstructions ? { extraInstructions } : {}),
  })
  if (!decision.ok || !decision.envelope) {
    console.log('follow skip', input.storyId, lane, !decision.ok ? decision.code : 'no envelope')
    return null
  }

  const contract = gateSmithEnvelope({
    lane,
    story: merged,
    executionTarget: 'DEV',
    envelope: decision.envelope,
    registry,
  })
  if (contract && !contract.ok) {
    console.log(
      'follow skip', input.storyId, lane, 'execution-contract',
      executionContractFailureText(contract) ?? contract.code,
    )
    return null
  }

  await input.enqueue(enqueueInput(input.storyId, decision.envelope))
  return lane
}
