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

const ASSAY_FAILURE_EVIDENCE = /\b(fail(?:ed|ure|ures|ing)?|violation|policy)\b/i

/**
 * ENG-FORGE-V4-08: run the execution-contract gate for a Smith envelope at the
 * hydration/enqueue boundary. Returns `null` when the gate does not apply
 * (non-Smith lane); otherwise the gate verdict. A failing verdict means the
 * envelope must NOT be queued — Smith never launches on a partial contract.
 */
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
    // Live factory registry by default; callers may inject a deterministic one.
    registry: input.registry ?? createAgentRuntimeRegistry(),
    field: smithFieldFacts(),
  })
}

/**
 * ENG-FORGE-V3-01: Assay is clean only when the run reports Complete and its
 * test evidence contains no failure/violation/policy marker. Missing test
 * evidence is not invented; resultStatus remains the primary completion fact.
 */
export function isCleanAssayResult(input: {
  resultStatus?: string | null
  testsSummary?: string | null
}): boolean {
  if (!/^complete$/i.test((input.resultStatus ?? '').trim())) return false
  return !ASSAY_FAILURE_EVIDENCE.test(input.testsSummary ?? '')
}

export function assayFailureEvidence(input: {
  testsSummary?: string | null
  failedCommands?: string[] | null
}): string | null {
  const summary = (input.testsSummary ?? '').trim()
  const commands = (input.failedCommands ?? []).map((command) => command.trim()).filter(Boolean)
  if (commands.length === 0) return summary || null
  const commandEvidence = `failed commands: ${commands.join(', ')}`
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
    // ENG-FORGE-V3-02 defense at the envelope boundary: even if lane-picking
    // regresses later, hydration cannot create Smith without a real brief.
    if (lane === 'smith' && !merged.architectBrief?.trim()) lane = 'scout'
    const decision = buildLaneEnqueue({
      lane,
      story: merged,
      registry,
    })
    if (!decision.ok || !decision.envelope) {
      console.log(
        'hydrate skip',
        item.storyId,
        decision.ok ? 'no envelope' : decision.code,
      )
      continue
    }
    // ENG-FORGE-V4-08: an incomplete Smith contract must not be stamped. The
    // gate sees the exact envelope this hydration would persist (target from
    // the bare item, defaulted to DEV exactly as the enqueue below does).
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
  /** Optional runtime registry for the gate; defaults to the live factory registry. */
  registry?: AgentRuntimeRegistry
}): Promise<string | null> {
  // Assay/reviewer is a terminal verification lane for this V3 slice. A failed
  // verification never grows and, importantly, cannot be treated as a shipped
  // success merely because the worker itself reached Done.
  if (input.finishedRole === 'reviewer') {
    if (!isCleanAssayResult(input)) return null
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

  // ENG-FORGE-V3-03: the first no-brief Ready hydration may Scout, but Scout
  // cannot wake itself forever. After Scout completes, a real architect brief
  // from Neon or the git packet must exist before the loop can advance to Smith.
  if (input.finishedRole === 'scout' && !merged.architectBrief?.trim()) {
    console.log('follow skip', input.storyId, 'scout', 'missing-architect-brief')
    return null
  }

  const lane = pickLane({ story: merged, lastFinishedRole: input.finishedRole })
  const decision = buildLaneEnqueue({ lane, story: merged })
  if (!decision.ok || !decision.envelope) {
    console.log(
      'follow skip',
      input.storyId,
      lane,
      !decision.ok ? decision.code : 'no envelope',
    )
    return null
  }
  // ENG-FORGE-V4-08: Scout→Smith may advance only when the merged packet plus
  // the resolved runtime assignment satisfy the full execution contract.
  // Follow persists the DEV target explicitly, so the gate sees 'DEV'.
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
