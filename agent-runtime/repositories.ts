// ---------------------------------------------------------------------------
// Repository layer for the Agent Runtime (ENG-18).
// ---------------------------------------------------------------------------

import {
  beginAgentWorkRun,
  cancelAgentWork,
  claimNextAgentWork,
  claimSpecificAgentWork,
  enqueueAgentWorkCommand,
  finishAgentWork,
  getAgentWorkItem,
  setAgentWorkRuntime,
  updateAgentWorkProgress,
  type AgentWorkClaim,
  type AgentWorkItem,
} from '../db/agent-work'
import { recoverAgentWorkInterruption } from '../db/agent-work-recovery'
import {
  getForgeRunExecutionStory,
  initializeForgeStoryRun,
  recordForgeLeadDecision,
  recordForgeRunMachineEvidence,
  setForgeRunRuntime,
} from '../db/forge-run'
import { markForgeStoryInProgress } from '../db/forge-story-state'
import {
  getStoryboardStory,
  listStoryRuns,
  startStoryRun,
  updateStoryRunProgress,
  type StoryboardStory,
  type StoryRun,
} from '../db/storyboard'
import type { QueryExecutor } from '../db/query-executor'
import type { AgentProgressUpdate } from './types'
import type { AssayEvidence } from './assay-evidence'
import {
  leadRunPhaseFromInstructions,
  parseLeadDecision,
} from './lead-decision'
import { runMachineEvidenceFromFinish } from './run-machine-evidence'
import {
  assayHoldEvidenceLine,
  candidateVerifiedEvidenceLine,
  isAssayTerminalRole,
  isCleanAssayEvidence,
  smithCandidateSha,
  verifiedShaFromWorkspaceEvidence,
} from './candidate-assay-handoff'

export type AssayFinishContext = {
  candidateSha: string | null
}

function runTypeForWorkItem(
  item: Pick<AgentWorkItem, 'role' | 'specialInstructions'>,
): string | null {
  // Lane identity comes from Forge's own machine-authored envelope, never from
  // whichever model/profile happened to be mapped into the position.
  const lane = (item.specialInstructions ?? '').match(
    /\bLane=(scout|architect|lead|smith|inspector|assay|archive|night)\b/,
  )?.[1]
  if (lane) return lane

  // Legacy work items predate the lane preamble. Preserve the nearest factual
  // role mapping without consulting model/profile identity.
  const role = (item.role ?? '').trim()
  if (role === 'lead') return 'lead'
  if (role === 'builder') return 'smith'
  if (role === 'verifier') return 'assay'
  if (role === 'reviewer') return 'inspector'
  if (role === 'scout') return 'scout'
  if (role === 'architect') return 'architect'
  return role || null
}

export function normalizeAgentFinishForRole(
  role: string | null,
  input: {
    resultStatus: string
    completion: number
    notes: string
    commitHash: string | null
    testsSummary: string | null
    assayEvidence?: AssayEvidence | null
    modelUsed?: string | null
  },
  context?: AssayFinishContext | null,
): {
  resultStatus: string
  completion: number
  notes: string
  commitHash: string | null
  testsSummary: string | null
  assayEvidence?: AssayEvidence | null
  modelUsed?: string | null
} {
  if (!isAssayTerminalRole(role)) return input

  const structured = input.assayEvidence ?? null
  const cleanEvidence = structured
    ? structured.verdict === 'PASS' && structured.failureCode === null
    : isCleanAssayEvidence({
        resultStatus: input.resultStatus,
        testsSummary: input.testsSummary,
      })

  if (!context) {
    if (!cleanEvidence) {
      return { ...input, resultStatus: 'Hold', commitHash: null }
    }
    return { ...input, commitHash: null }
  }

  const candidateSha = smithCandidateSha([{ commitHash: context.candidateSha }])
  const verifiedSha =
    structured?.verifiedSha ?? verifiedShaFromWorkspaceEvidence(input.notes)
  const structuredCandidateMatches = structured
    ? structured.candidateSha === candidateSha
    : true
  const verifiedExactCandidate = Boolean(
    cleanEvidence &&
      candidateSha &&
      verifiedSha === candidateSha &&
      structuredCandidateMatches,
  )

  if (verifiedExactCandidate) {
    return {
      ...input,
      notes: [input.notes.trim(), candidateVerifiedEvidenceLine(candidateSha!)]
        .filter(Boolean)
        .join('\n\n'),
      commitHash: null,
    }
  }

  const structuredFailure = structured?.failureCode
    ? `Assay Hold: ${structured.failureCode}: ${structured.failureDetail ?? 'structured verification failed.'}`
    : null
  const compatibilityEvidence = assayHoldEvidenceLine({
    candidateSha,
    verifiedSha,
    cleanEvidence,
  })
  return {
    ...input,
    resultStatus: 'Hold',
    notes: [input.notes.trim(), structuredFailure ?? compatibilityEvidence]
      .filter(Boolean)
      .join('\n\n'),
    commitHash: null,
  }
}

export interface AgentWorkRepository {
  claimNext(workerId: string): Promise<AgentWorkClaim | null>
  claimSpecific(workItemId: string, workerId: string): Promise<AgentWorkItem | null>
  get(workItemId: string): Promise<AgentWorkItem | null>
  enqueue(input: {
    storyId: string
    role?: string | null
    modelProfile?: string | null
    specialInstructions?: string | null
    priority?: number
    maxAttempts?: number
    executionPolicy?: string
    executionEnvironment?: string | null
  }): Promise<AgentWorkItem>
  beginRun(workItemId: string): Promise<{ workItem: AgentWorkItem; story: StoryboardStory }>
  progress(workItemId: string, input: AgentProgressUpdate): Promise<AgentWorkItem>
  setRuntime(
    workItemId: string,
    input: { runtimeAdapter: string; externalRunId?: string | null },
  ): Promise<AgentWorkItem>
  finish(
    workItemId: string,
    input: {
      resultStatus: string
      completion: number
      notes: string
      commitHash: string | null
      testsSummary: string | null
      assayEvidence?: AssayEvidence | null
    },
  ): Promise<{ workItem: AgentWorkItem; run: unknown; story: StoryboardStory }>
  fail(workItemId: string, errorText: string): Promise<AgentWorkItem>
  cancel(workItemId: string, note?: string): Promise<AgentWorkItem>
}

export interface AgentRunRepository {
  start(storyId: string): Promise<{ run: StoryRun; story: StoryboardStory }>
  progress(runId: string, input: AgentProgressUpdate): Promise<StoryRun>
  listForStory(storyId: string): Promise<StoryRun[]>
}

export interface StoryContextRepository {
  getStory(storyId: string): Promise<StoryboardStory | null>
}

export type ExecutorProvider = () => Promise<QueryExecutor>

export class SqlAgentWorkRepository implements AgentWorkRepository {
  constructor(private readonly executor: ExecutorProvider) {}

  async claimNext(workerId: string): Promise<AgentWorkClaim | null> {
    return claimNextAgentWork(workerId)
  }

  async claimSpecific(workItemId: string, workerId: string): Promise<AgentWorkItem | null> {
    return claimSpecificAgentWork(workItemId, workerId)
  }

  async get(workItemId: string): Promise<AgentWorkItem | null> {
    const q = await this.executor()
    return getAgentWorkItem(workItemId, q)
  }

  async enqueue(input: {
    storyId: string
    role?: string | null
    modelProfile?: string | null
    specialInstructions?: string | null
    priority?: number
    maxAttempts?: number
    executionPolicy?: string
    executionEnvironment?: string | null
  }) {
    const q = await this.executor()
    return enqueueAgentWorkCommand(input, q)
  }

  async beginRun(workItemId: string) {
    const q = await this.executor()
    const begun = await beginAgentWorkRun(workItemId, q)
    const runId = begun.workItem.storyRunId
    if (!runId) {
      throw new Error(`work item ${workItemId} began without a Story Run id`)
    }

    const runPhase =
      begun.workItem.role === 'lead'
        ? leadRunPhaseFromInstructions(begun.workItem.specialInstructions)
        : null
    await initializeForgeStoryRun(
      runId,
      {
        runType: runTypeForWorkItem(begun.workItem),
        runPhase,
        agentRuntime: begun.workItem.runtimeAdapter ?? null,
      },
      q,
    )

    const executionStory = await getForgeRunExecutionStory(runId, begun.story, q)
    if (!executionStory) {
      throw new Error(`Story Run ${runId} disappeared before execution context could be loaded`)
    }

    return { ...begun, story: executionStory }
  }

  async progress(workItemId: string, input: AgentProgressUpdate) {
    const q = await this.executor()
    await updateAgentWorkProgress(
      workItemId,
      {
        completion: input.completion,
        note: input.note,
        testsSummary: input.testsSummary ?? null,
      },
      q,
    )
    return (await getAgentWorkItem(workItemId, q))!
  }

  async setRuntime(
    workItemId: string,
    input: { runtimeAdapter: string; externalRunId?: string | null },
  ) {
    const q = await this.executor()
    const item = await setAgentWorkRuntime(workItemId, input, q)
    if (item.storyRunId) {
      await setForgeRunRuntime(item.storyRunId, input.runtimeAdapter, q)
    }
    return item
  }

  async finish(
    workItemId: string,
    input: {
      resultStatus: string
      completion: number
      notes: string
      commitHash: string | null
      testsSummary: string | null
      assayEvidence?: AssayEvidence | null
      modelUsed?: string | null
    },
  ): Promise<{ workItem: AgentWorkItem; run: unknown; story: StoryboardStory }> {
    const q = await this.executor()
    const item = await getAgentWorkItem(workItemId, q)
    let context: AssayFinishContext | undefined
    if (item && isAssayTerminalRole(item.role)) {
      const runs = await listStoryRuns(item.storyId, q)
      context = { candidateSha: smithCandidateSha(runs) }
    }

    const normalized = normalizeAgentFinishForRole(item?.role ?? null, input, context)
    const machineEvidence = {
      ...runMachineEvidenceFromFinish({
        role: item?.role ?? null,
        resultStatus: normalized.resultStatus,
        notes: normalized.notes,
        testsSummary: normalized.testsSummary,
        assayEvidence: normalized.assayEvidence ?? input.assayEvidence ?? null,
      }),
      // Spend vision: harness-observed model identity, never model self-report.
      modelUsed: normalized.modelUsed ?? input.modelUsed ?? null,
    }

    const finished = await finishAgentWork(
      workItemId,
      {
        resultStatus: normalized.resultStatus,
        completion: normalized.completion,
        notes: normalized.notes,
        commitHash: normalized.commitHash,
        testsSummary: normalized.testsSummary,
      },
      q,
    )

    if (item?.storyRunId) {
      await recordForgeRunMachineEvidence(item.storyRunId, machineEvidence, q)

      if (item.role === 'lead') {
        const phase = leadRunPhaseFromInstructions(item.specialInstructions)
        if (phase === 'pre' || phase === 'post') {
          const parsed = parseLeadDecision(normalized.notes)
          const decision = parsed?.decision ?? 'HOLD'
          const detail = parsed?.reason ??
            `Lead ${phase.toUpperCase()} did not produce a valid structured decision; fail closed.`
          await recordForgeLeadDecision(
            item.storyRunId,
            {
              phase,
              decision,
              splitCount: parsed?.splitCount ?? null,
              detail: `Lead ${phase.toUpperCase()} decision=${decision}${parsed?.splitCount ? `:${parsed.splitCount}` : ''} — ${detail}`,
            },
            q,
          )
        }
      }
    }

    // Intermediate lanes never own final Story completion. Lead PRE/IMPLEMENT/
    // POST and Smith all hand off to another gate; a clean Assay still waits
    // for outer publish to own final Complete.
    const intermediateAwaitingFollow = Boolean(
      item &&
        (item.role === 'builder' || item.role === 'lead' || item.role === 'architect') &&
        /^complete$/i.test(normalized.resultStatus),
    )
    const assayAwaitingPublish = Boolean(
      item &&
        isAssayTerminalRole(item.role) &&
        input.assayEvidence?.verdict === 'PASS' &&
        /^complete$/i.test(normalized.resultStatus),
    )
    if (item && (intermediateAwaitingFollow || assayAwaitingPublish)) {
      await markForgeStoryInProgress(item.storyId, q)
      const story: StoryboardStory = {
        ...finished.story,
        status: 'In Progress',
        completedAt: null,
      }
      return { ...finished, story }
    }

    return finished
  }

  async fail(workItemId: string, errorText: string) {
    const q = await this.executor()
    const recovered = await recoverAgentWorkInterruption(workItemId, errorText, q)
    return recovered.workItem
  }

  async cancel(workItemId: string, note?: string) {
    const q = await this.executor()
    return cancelAgentWork(workItemId, { note }, q)
  }
}

export class SqlAgentRunRepository implements AgentRunRepository {
  constructor(private readonly executor: ExecutorProvider) {}

  async start(storyId: string) {
    const q = await this.executor()
    const started = await startStoryRun(storyId, q)
    await initializeForgeStoryRun(
      started.run.id,
      { runType: null, runPhase: null, agentRuntime: null },
      q,
    )
    const executionStory = await getForgeRunExecutionStory(started.run.id, started.story, q)
    if (!executionStory) {
      throw new Error(`Story Run ${started.run.id} disappeared before execution context could be loaded`)
    }
    return { ...started, story: executionStory }
  }

  async progress(runId: string, input: AgentProgressUpdate) {
    const q = await this.executor()
    return updateStoryRunProgress(
      runId,
      {
        completion: input.completion,
        note: input.note,
        testsSummary: input.testsSummary ?? null,
      },
      q,
    )
  }

  async listForStory(storyId: string): Promise<StoryRun[]> {
    const q = await this.executor()
    return listStoryRuns(storyId, q)
  }
}

export class SqlStoryContextRepository implements StoryContextRepository {
  constructor(private readonly executor: ExecutorProvider) {}

  async getStory(storyId: string): Promise<StoryboardStory | null> {
    const q = await this.executor()
    return getStoryboardStory(storyId, q)
  }
}
