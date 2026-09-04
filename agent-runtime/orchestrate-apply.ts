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
import { decideForgeTransition } from './forge-transition'

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
  /** Optional runtime registry for the gate; defaults to the live factory registry. */
  registry?: AgentRuntimeRegistry
}

/** One deterministic preflight gate before Smith token spend. */
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

/** Legacy compatibility for pre-V6 Assay rows. New V6 Assay decisions use the
 * structured assay.verdict event and never derive truth from this summary. */
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
    let lane = pickLane({ story: merged })
    if (lane === 'smith' && !merged.architectBrief?.trim()) lane = 'scout'
    const decision = buildLaneEnqueue({ lane, story: merged, registry })
    if (!decision.ok || !decision.envelope) {
      console.log(
        'hydrate skip',
        item.storyId,
        decision.ok ? 'no envelope' : decision.code,
      )
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
}): Promise<string | null> {
  // Assay is terminal for automation. A PASS goes to the separate publish seam;
  // a FAIL goes to human Hold. Neither outcome may enqueue another lane here.
  if (isAssayTerminalRole(input.finishedRole)) {
    const clean = isCleanAssayResult(input)
    const transition = decideForgeTransition(
      clean
        ? { type: 'assay-pass' }
        : {
            type: 'assay-fail',
            code: 'ASSAY_TEST_FAILED',
            detail: 'Assay did not produce a clean verification result.',
          },
    )
    if (transition.action === 'publish' || transition.action === 'hold-human') {
      return null
    }
    return null
  }

  const ok =
    !input.resultStatus || /complete|success|pass/i.test(input.resultStatus)
  if (!ok) return null
  if (input.finishedRole !== 'builder' && input.finishedRole !== 'scout') {
    return null
  }
  const story = await input.getStory(input.storyId)
  if (!story) return null
  const merged = storyFieldsFromBoardAndGit(story, input.storyId, input.repoRoot)

  if (input.finishedRole === 'scout' && !merged.architectBrief?.trim()) {
    console.log('follow skip', input.storyId, 'scout', 'missing-architect-brief')
    return null
  }

  const lane = pickLane({ story: merged, lastFinishedRole: input.finishedRole })
  const candidateSha = (input.candidateSha ?? '').trim() || null

  if (input.finishedRole === 'builder') {
    const transition = decideForgeTransition({
      type: 'smith-complete',
      candidateSha,
    })
    if (transition.action !== 'enqueue-assay') {
      console.log(
        'follow skip',
        input.storyId,
        'assay',
        transition.failure?.code ?? 'transition-stop',
        transition.failure?.detail ?? '',
      )
      return null
    }
  }

  if (lane === 'assay' && !candidateSha) return null

  const extraInstructions =
    lane === 'assay' && candidateSha
      ? withAssayCandidateDirective(
          `Forge V6 exact-candidate Assay: execute the immutable Assay plan against Smith candidate ${candidateSha}. The worktree HEAD MUST equal this SHA. Any command failure, policy violation, or SHA mismatch is a human intervention point; never restart Smith automatically.`,
          candidateSha,
        )
      : null

  const decision = buildLaneEnqueue({
    lane,
    story: merged,
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
