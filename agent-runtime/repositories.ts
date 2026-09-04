// ---------------------------------------------------------------------------
// Repository layer for the Agent Runtime (ENG-18 / Forge V6.1).
// ---------------------------------------------------------------------------

import {
  beginAgentWorkRun,
  cancelAgentWork,
  finishAgentWork,
  setAgentWorkRuntime,
  updateAgentWorkProgress,
} from '../db/agent-work'
import {
  claimNextForgeAgentWork,
  claimSpecificForgeAgentWork,
  enqueueForgeAgentWorkCommand,
  getForgeAgentWorkItem,
  validateForgeWorkRouting,
  type EnqueueForgeAgentWorkInput,
  type ForgeAgentWorkClaim,
  type ForgeAgentWorkItem,
} from '../db/agent-work-v61'
import { recoverAgentWorkInterruption } from '../db/agent-work-recovery'
import {
  appendForgeRunDetail,
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
import { leadRunPhaseFromInstructions, parseLeadDecision } from './lead-decision'
import { parseQaDecision } from './qa-decision'
import { runMachineEvidenceFromFinish } from './run-machine-evidence'
import {
  assayHoldEvidenceLine,
  candidateVerifiedEvidenceLine,
  isAssayTerminalRole,
  isCleanAssayEvidence,
  smithCandidateSha,
  verifiedShaFromWorkspaceEvidence,
} from './candidate-assay-handoff'

export type AssayFinishContext = { candidateSha: string | null }

type FinishInput = {
  resultStatus: string
  completion: number
  notes: string
  commitHash: string | null
  testsSummary: string | null
  assayEvidence?: AssayEvidence | null
}

function runTypeForWorkItem(item: ForgeAgentWorkItem): string | null {
  if (item.lane) return item.lane
  const legacy = (item.specialInstructions ?? '').match(
    /\bLane=(scout|architect|lead|smith|inspector|assay|archive|night)\b/,
  )?.[1]
  if (legacy) return legacy
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
  input: FinishInput,
  context?: AssayFinishContext | null,
): FinishInput {
  if (!isAssayTerminalRole(role)) return input

  const structured = input.assayEvidence ?? null
  const cleanEvidence = structured
    ? structured.verdict === 'PASS' && structured.failureCode === null
    : isCleanAssayEvidence({ resultStatus: input.resultStatus, testsSummary: input.testsSummary })

  if (!context) {
    return cleanEvidence ? { ...input, commitHash: null } : { ...input, resultStatus: 'Hold', commitHash: null }
  }

  const candidateSha = smithCandidateSha([{ commitHash: context.candidateSha }])
  const verifiedSha = structured?.verifiedSha ?? verifiedShaFromWorkspaceEvidence(input.notes)
  const structuredCandidateMatches = structured ? structured.candidateSha === candidateSha : true
  const verifiedExactCandidate = Boolean(
    cleanEvidence && candidateSha && verifiedSha === candidateSha && structuredCandidateMatches,
  )

  if (verifiedExactCandidate) {
    return {
      ...input,
      notes: [input.notes.trim(), candidateVerifiedEvidenceLine(candidateSha!)].filter(Boolean).join('\n\n'),
      commitHash: null,
    }
  }

  const structuredFailure = structured?.failureCode
    ? `Assay Hold: ${structured.failureCode}: ${structured.failureDetail ?? 'structured verification failed.'}`
    : null
  const compatibilityEvidence = assayHoldEvidenceLine({ candidateSha, verifiedSha, cleanEvidence })
  return {
    ...input,
    resultStatus: 'Hold',
    notes: [input.notes.trim(), structuredFailure ?? compatibilityEvidence].filter(Boolean).join('\n\n'),
    commitHash: null,
  }
}

function normalizeQaFinish(item: ForgeAgentWorkItem, input: FinishInput): {
  finish: FinishInput
  failureCode: string | null
} {
  if (item.lane !== 'inspector' && item.role !== 'reviewer') {
    return { finish: input, failureCode: null }
  }
  const decision = parseQaDecision(input.notes)
  if (!decision) {
    return {
      finish: {
        ...input,
        resultStatus: 'Hold',
        commitHash: null,
        notes: `${input.notes.trim()}\n\nQA Hold: missing valid QA_DECISION: ASSAY | HOLD.`.trim(),
      },
      failureCode: 'QA_DECISION_MISSING',
    }
  }
  if (decision.decision === 'HOLD') {
    return {
      finish: {
        ...input,
        resultStatus: 'Hold',
        commitHash: null,
        notes: `${input.notes.trim()}\n\nQA Hold: ${decision.reason ?? 'independent review rejected the candidate.'}`.trim(),
      },
      failureCode: 'QA_REVIEW_FAILED',
    }
  }
  return {
    finish: { ...input, resultStatus: 'Complete', commitHash: null },
    failureCode: null,
  }
}

export interface AgentWorkRepository {
  claimNext(workerId: string): Promise<ForgeAgentWorkClaim | null>
  claimSpecific(workItemId: string, workerId: string): Promise<ForgeAgentWorkItem | null>
  get(workItemId: string): Promise<ForgeAgentWorkItem | null>
  enqueue(input: EnqueueForgeAgentWorkInput): Promise<ForgeAgentWorkItem>
  beginRun(workItemId: string): Promise<{ workItem: ForgeAgentWorkItem; story: StoryboardStory }>
  progress(workItemId: string, input: AgentProgressUpdate): Promise<ForgeAgentWorkItem>
  setRuntime(
    workItemId: string,
    input: { runtimeAdapter: string; externalRunId?: string | null },
  ): Promise<ForgeAgentWorkItem>
  finish(
    workItemId: string,
    input: FinishInput,
  ): Promise<{ workItem: ForgeAgentWorkItem; run: unknown; story: StoryboardStory }>
  fail(workItemId: string, errorText: string): Promise<ForgeAgentWorkItem>
  cancel(workItemId: string, note?: string): Promise<ForgeAgentWorkItem>
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

  async claimNext(workerId: string): Promise<ForgeAgentWorkClaim | null> {
    return claimNextForgeAgentWork(workerId)
  }

  async claimSpecific(workItemId: string, workerId: string): Promise<ForgeAgentWorkItem | null> {
    return claimSpecificForgeAgentWork(workItemId, workerId)
  }

  async get(workItemId: string): Promise<ForgeAgentWorkItem | null> {
    return getForgeAgentWorkItem(workItemId, await this.executor())
  }

  async enqueue(input: EnqueueForgeAgentWorkInput) {
    return enqueueForgeAgentWorkCommand(input, await this.executor())
  }

  async beginRun(workItemId: string) {
    const q = await this.executor()
    const before = await getForgeAgentWorkItem(workItemId, q)
    if (!before) throw new Error(`work item ${workItemId} was not found`)
    const routingError = validateForgeWorkRouting(before)
    if (routingError) throw new Error(`cannot launch work item ${workItemId}: ${routingError}`)

    const begun = await beginAgentWorkRun(workItemId, q)
    const item = await getForgeAgentWorkItem(workItemId, q)
    if (!item?.storyRunId) throw new Error(`work item ${workItemId} began without a Story Run id`)
    const runId = item.storyRunId
    const runPhase = item.runPhase ??
      (item.role === 'lead' ? leadRunPhaseFromInstructions(item.specialInstructions) : null)

    await initializeForgeStoryRun(
      runId,
      { runType: runTypeForWorkItem(item), runPhase, agentRuntime: item.runtimeAdapter ?? null },
      q,
    )
    await appendForgeRunDetail(
      runId,
      `assignment lane=${item.lane ?? '(legacy)'} phase=${runPhase ?? '(none)'} profile=${item.modelProfile ?? '(none)'} player=${item.playerId ?? '(none)'} provider=${item.providerId ?? '(none)'} model=${item.modelId ?? '(none)'} harness=${item.harnessId ?? '(none)'} field=${item.fieldId ?? '(none)'}`,
      q,
    )

    const executionStory = await getForgeRunExecutionStory(runId, begun.story, q)
    if (!executionStory) throw new Error(`Story Run ${runId} disappeared before execution context could be loaded`)
    return { workItem: item, story: executionStory }
  }

  async progress(workItemId: string, input: AgentProgressUpdate) {
    const q = await this.executor()
    await updateAgentWorkProgress(
      workItemId,
      { completion: input.completion, note: input.note, testsSummary: input.testsSummary ?? null },
      q,
    )
    const item = await getForgeAgentWorkItem(workItemId, q)
    if (!item) throw new Error(`work item ${workItemId} disappeared`)
    return item
  }

  async setRuntime(
    workItemId: string,
    input: { runtimeAdapter: string; externalRunId?: string | null },
  ) {
    const q = await this.executor()
    const base = await setAgentWorkRuntime(workItemId, input, q)
    if (base.storyRunId) await setForgeRunRuntime(base.storyRunId, input.runtimeAdapter, q)
    const item = await getForgeAgentWorkItem(workItemId, q)
    if (!item) throw new Error(`work item ${workItemId} disappeared`)
    return item
  }

  async finish(
    workItemId: string,
    input: FinishInput,
  ): Promise<{ workItem: ForgeAgentWorkItem; run: unknown; story: StoryboardStory }> {
    const q = await this.executor()
    const item = await getForgeAgentWorkItem(workItemId, q)
    let assayContext: AssayFinishContext | undefined
    if (item && isAssayTerminalRole(item.role)) {
      assayContext = { candidateSha: item.candidateShas[0] ?? null }
    }

    const assayNormalized = normalizeAgentFinishForRole(item?.role ?? null, input, assayContext)
    const qaNormalized = item ? normalizeQaFinish(item, assayNormalized) : { finish: assayNormalized, failureCode: null }
    const normalized = qaNormalized.finish
    const machineEvidence = runMachineEvidenceFromFinish({
      role: item?.role ?? null,
      resultStatus: normalized.resultStatus,
      notes: normalized.notes,
      testsSummary: normalized.testsSummary,
      assayEvidence: normalized.assayEvidence ?? input.assayEvidence ?? null,
    })
    if (qaNormalized.failureCode) machineEvidence.failureCode = qaNormalized.failureCode

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

    const after = await getForgeAgentWorkItem(workItemId, q)
    if (!after) throw new Error(`work item ${workItemId} disappeared after finish`)

    if (item?.storyRunId) {
      await recordForgeRunMachineEvidence(item.storyRunId, machineEvidence, q)
      if (item.role === 'lead') {
        const phase = item.runPhase ?? leadRunPhaseFromInstructions(item.specialInstructions)
        if (phase === 'pre' || phase === 'post') {
          const parsed = parseLeadDecision(normalized.notes)
          const decision = parsed?.decision ?? 'HOLD'
          const detail = parsed?.reason ?? `Lead ${phase.toUpperCase()} did not produce a valid structured decision; fail closed.`
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

    const intermediateAwaitingFollow = Boolean(
      item &&
        ['builder', 'lead', 'architect', 'scout', 'reviewer'].includes(item.role ?? '') &&
        /^complete$/i.test(normalized.resultStatus),
    )
    const assayAwaitingPublish = Boolean(
      item && isAssayTerminalRole(item.role) && input.assayEvidence?.verdict === 'PASS' && /^complete$/i.test(normalized.resultStatus),
    )
    if (item && (intermediateAwaitingFollow || assayAwaitingPublish)) {
      await markForgeStoryInProgress(item.storyId, q)
      return {
        workItem: after,
        run: finished.run,
        story: { ...finished.story, status: 'In Progress', completedAt: null },
      }
    }

    return { workItem: after, run: finished.run, story: finished.story }
  }

  async fail(workItemId: string, errorText: string) {
    const q = await this.executor()
    await recoverAgentWorkInterruption(workItemId, errorText, q)
    const item = await getForgeAgentWorkItem(workItemId, q)
    if (!item) throw new Error(`work item ${workItemId} disappeared after recovery`)
    return item
  }

  async cancel(workItemId: string, note?: string) {
    const q = await this.executor()
    await cancelAgentWork(workItemId, { note }, q)
    const item = await getForgeAgentWorkItem(workItemId, q)
    if (!item) throw new Error(`work item ${workItemId} disappeared after cancellation`)
    return item
  }
}

export class SqlAgentRunRepository implements AgentRunRepository {
  constructor(private readonly executor: ExecutorProvider) {}

  async start(storyId: string) {
    const q = await this.executor()
    const started = await startStoryRun(storyId, q)
    await initializeForgeStoryRun(started.run.id, { runType: null, runPhase: null, agentRuntime: null }, q)
    const executionStory = await getForgeRunExecutionStory(started.run.id, started.story, q)
    if (!executionStory) throw new Error(`Story Run ${started.run.id} disappeared before execution context could be loaded`)
    return { ...started, story: executionStory }
  }

  async progress(runId: string, input: AgentProgressUpdate) {
    const q = await this.executor()
    return updateStoryRunProgress(
      runId,
      { completion: input.completion, note: input.note, testsSummary: input.testsSummary ?? null },
      q,
    )
  }

  async listForStory(storyId: string): Promise<StoryRun[]> {
    return listStoryRuns(storyId, await this.executor())
  }
}

export class SqlStoryContextRepository implements StoryContextRepository {
  constructor(private readonly executor: ExecutorProvider) {}
  async getStory(storyId: string): Promise<StoryboardStory | null> {
    return getStoryboardStory(storyId, await this.executor())
  }
}
