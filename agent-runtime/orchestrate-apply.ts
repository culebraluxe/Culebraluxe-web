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
import { smithFieldFacts } from './team'
import {
  isAssayTerminalRole,
  isCleanAssayEvidence,
} from './candidate-assay-handoff'
import { withAssayCandidateDirective } from './assay-evidence'
import {
  decideForgeTransition,
  type ForgeTransitionDecision,
} from './forge-transition'
import type {
  LeadDecisionCode,
  LeadRunPhase,
} from './lead-decision'

export type BareWorkItem = {
  id: string
  storyId: string
  state: string
  role: string | null
  modelProfile: string | null
  executionEnvironment: string | null
  executionPolicy: string
  priority: number
}

export type HydrateDeps = {
  listItems: () => Promise<BareWorkItem[] | null>
  getStory: (id: string) => Promise<StoryPacketFields | null>
  enqueue: (input: {
    storyId: string
    role: string
    modelProfile: string
    specialInstructions: string | null
    priority?: number
    maxAttempts?: number
    executionPolicy?: string
    executionEnvironment?: string | null
  }) => Promise<unknown>
  repoRoot?: string
  registry?: AgentRuntimeRegistry
}

function gateSmithEnvelope(input: {
  lane: LaneId
  story: StoryPacketFields
  executionTarget: string | null | undefined
  envelope: LaneEnqueueEnvelope
  registry?: AgentRuntimeRegistry
}): ExecutionContractResult | null {
  if (input.lane !== 'smith') return null
  return validateExecutionContract({
    story: input.story,
    executionTarget: input.executionTarget,
    modelProfile: input.envelope.modelProfile,
    registry: input.registry ?? createAgentRuntimeRegistry(),
    field: smithFieldFacts(),
  })
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
    (item) => item.state === 'Ready' && (!item.role || !item.modelProfile),
  )
  const stamped: string[] = []
  const registry = deps.registry ?? createAgentRuntimeRegistry()
  for (const item of bare) {
    const story = await deps.getStory(item.storyId)
    if (!story) continue
    const merged = storyFieldsFromBoardAndGit(story, item.storyId, deps.repoRoot)
    const lane = pickLane({ story: merged })
    const decision = buildLaneEnqueue({
      lane,
      story: merged,
      registry,
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
        'hydrate skip',
        item.storyId,
        'smith',
        'execution-contract',
        executionContractFailureText(contract) ?? contract.code,
      )
      continue
    }
    await deps.enqueue({
      storyId: item.storyId,
      role: decision.envelope.role,
      modelProfile: decision.envelope.modelProfile,
      specialInstructions: decision.envelope.specialInstructions,
      priority: item.priority,
      maxAttempts: decision.envelope.maxAttempts,
      executionPolicy: item.executionPolicy || 'Unattended OK',
      executionEnvironment: item.executionEnvironment ?? 'DEV',
    })
    stamped.push(`${item.storyId}:${lane}`)
  }
  return stamped
}

function transitionForFinishedLane(input: {
  finishedRole: string
  candidateSha: string | null
  leadPhase?: LeadRunPhase | null
  leadDecision?: LeadDecisionCode | null
  leadSplitCount?: number | null
  leadReason?: string | null
}): ForgeTransitionDecision | null {
  if (input.finishedRole === 'architect' || input.finishedRole === 'scout') {
    return decideForgeTransition({ type: 'architect-complete' })
  }
  if (input.finishedRole === 'builder') {
    return decideForgeTransition({
      type: 'smith-complete',
      candidateSha: input.candidateSha,
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
      return decideForgeTransition({
        type: 'lead-implement-complete',
        candidateSha: input.candidateSha,
      })
    }
    if (input.leadPhase === 'post') {
      return decideForgeTransition({
        type: 'lead-post',
        decision: input.leadDecision ?? null,
        candidateSha: input.candidateSha,
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
  candidateSha?: string | null
  leadPhase?: LeadRunPhase | null
  leadDecision?: LeadDecisionCode | null
  leadSplitCount?: number | null
  leadReason?: string | null
}): Promise<string | null> {
  if (isAssayTerminalRole(input.finishedRole)) return null

  const ok =
    !input.resultStatus || /complete|success|pass/i.test(input.resultStatus)
  if (!ok) return null

  const story = await input.getStory(input.storyId)
  if (!story) return null
  const merged = storyFieldsFromBoardAndGit(story, input.storyId, input.repoRoot)

  if (input.finishedRole === 'scout' && !merged.architectBrief?.trim()) {
    console.log('follow skip', input.storyId, 'scout', 'missing-architect-brief')
    return null
  }

  const candidateSha = (input.candidateSha ?? '').trim() || null
  const transition = transitionForFinishedLane({
    finishedRole: input.finishedRole,
    candidateSha,
    leadPhase: input.leadPhase,
    leadDecision: input.leadDecision,
    leadSplitCount: input.leadSplitCount,
    leadReason: input.leadReason,
  })
  if (!transition || !transition.nextLane || transition.action === 'hold-human') {
    return null
  }

  const lane = transition.nextLane
  const leadPhase = transition.nextPhase ?? undefined
  const extraInstructions =
    lane === 'assay' && candidateSha
      ? withAssayCandidateDirective(
          `Forge V6 exact-candidate Assay: execute the immutable Assay plan against integrated candidate ${candidateSha}. The worktree HEAD MUST equal this SHA. Any command failure, policy violation, or SHA mismatch is a human intervention point; never restart Smith automatically.`,
          candidateSha,
        )
      : null

  const decision = buildLaneEnqueue({
    lane,
    story: merged,
    ...(leadPhase ? { leadPhase } : {}),
    ...(extraInstructions ? { extraInstructions } : {}),
  })
  if (!decision.ok || !decision.envelope) {
    console.log(
      'follow skip',
      input.storyId,
      lane,
      !decision.ok ? decision.code : 'no envelope',
    )
    return null
  }

  const contract = gateSmithEnvelope({
    lane,
    story: merged,
    executionTarget: 'DEV',
    envelope: decision.envelope,
    registry: input.registry,
  })
  if (contract && !contract.ok) {
    console.log(
      'follow skip',
      input.storyId,
      lane,
      'execution-contract',
      executionContractFailureText(contract) ?? contract.code,
    )
    return null
  }

  await input.enqueue({
    storyId: input.storyId,
    role: decision.envelope.role,
    modelProfile: decision.envelope.modelProfile,
    specialInstructions: decision.envelope.specialInstructions,
    executionEnvironment: 'DEV',
  })
  return lane
}
