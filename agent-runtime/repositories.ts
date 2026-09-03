// ---------------------------------------------------------------------------
// Repository layer for the Agent Runtime (ENG-18).
//
// The abstract AgentRuntimeAdapter and its concrete subclasses never issue raw
// SQL. All persisted lifecycle behavior flows through these narrow services,
// which delegate to the existing db/agent-work.ts and db/storyboard.ts
// repositories (no parallel queue/run/story systems).
// ---------------------------------------------------------------------------

import {
  beginAgentWorkRun,
  cancelAgentWork,
  claimNextAgentWork,
  claimSpecificAgentWork,
  enqueueAgentWorkCommand,
  failAgentWork,
  finishAgentWork,
  getAgentWorkItem,
  setAgentWorkRuntime,
  updateAgentWorkProgress,
  type AgentWorkClaim,
  type AgentWorkItem,
} from '../db/agent-work'
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

const ASSAY_FAILURE_EVIDENCE = /\b(fail(?:ed|ure|ures|ing)?|violation|policy)\b/i

export function normalizeAgentFinishForRole(
  role: string | null,
  input: {
    resultStatus: string
    completion: number
    notes: string
    commitHash: string | null
    testsSummary: string | null
  },
): {
  resultStatus: string
  completion: number
  notes: string
  commitHash: string | null
  testsSummary: string | null
} {
  if (role !== 'reviewer') return input
  const clean =
    /^complete$/i.test(input.resultStatus.trim()) &&
    !ASSAY_FAILURE_EVIDENCE.test(input.testsSummary ?? '')
  if (clean) {
    // Smith/builder remains the only writable lane. Assay never keeps a commit.
    return { ...input, commitHash: null }
  }
  return {
    ...input,
    resultStatus: 'Hold',
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

/** Production/DEV query executor provider (lazy so tests inject fakes). */
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
    return beginAgentWorkRun(workItemId, q)
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
    return setAgentWorkRuntime(workItemId, input, q)
  }

  async finish(
    workItemId: string,
    input: {
      resultStatus: string
      completion: number
      notes: string
      commitHash: string | null
      testsSummary: string | null
    },
  ) {
    const q = await this.executor()
    const item = await getAgentWorkItem(workItemId, q)
    const normalized = normalizeAgentFinishForRole(item?.role ?? null, input)
    return finishAgentWork(workItemId, normalized, q)
  }

  async fail(workItemId: string, errorText: string) {
    const q = await this.executor()
    return failAgentWork(workItemId, errorText, {}, q)
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
    return startStoryRun(storyId, q)
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
