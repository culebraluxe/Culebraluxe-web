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
import {
  assayHoldEvidenceLine,
  candidateVerifiedEvidenceLine,
  isAssayTerminalRole,
  isCleanAssayEvidence,
  smithCandidateSha,
  verifiedShaFromWorkspaceEvidence,
} from './candidate-assay-handoff'

/** Resolved Assay verification context (ENG-FORGE-V4-10C). When present, a
 *  clean Assay must additionally prove it executed against the exact Smith
 *  candidate commit — never a fallback base such as `main`. */
export type AssayFinishContext = {
  candidateSha: string | null
}

export function normalizeAgentFinishForRole(
  role: string | null,
  input: {
    resultStatus: string
    completion: number
    notes: string
    commitHash: string | null
    testsSummary: string | null
  },
  context?: AssayFinishContext | null,
): {
  resultStatus: string
  completion: number
  notes: string
  commitHash: string | null
  testsSummary: string | null
} {
  if (!isAssayTerminalRole(role)) return input
  const cleanEvidence = isCleanAssayEvidence({
    resultStatus: input.resultStatus,
    testsSummary: input.testsSummary,
  })

  // No resolved candidate context (legacy/direct callers): keep the original
  // status + failure-evidence semantics — clean Complete may survive (with no
  // commit) only when the evidence contains no failure marker.
  if (!context) {
    if (!cleanEvidence) {
      return { ...input, resultStatus: 'Hold', commitHash: null }
    }
    // Smith/builder remains the only writable lane. Assay never keeps a commit.
    return { ...input, commitHash: null }
  }

  // ENG-FORGE-V4-10C strict invariant: the Assay workspace base recorded in
  // the run evidence must be EXACTLY the Smith candidate this Assay was meant
  // to verify. Missing/unresolvable candidate, missing workspace evidence, or
  // a base that differs from the candidate fails closed to Hold — Assay never
  // silently falls back to `main` and a wrong-base verification is never
  // normalized to Complete.
  const candidateSha = smithCandidateSha([{ commitHash: context.candidateSha }])
  const verifiedSha = verifiedShaFromWorkspaceEvidence(input.notes)
  const verifiedExactCandidate = Boolean(
    cleanEvidence && candidateSha && verifiedSha === candidateSha,
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

  const evidence = assayHoldEvidenceLine({
    candidateSha,
    verifiedSha,
    cleanEvidence,
  })
  return {
    ...input,
    resultStatus: 'Hold',
    notes: [input.notes.trim(), evidence].filter(Boolean).join('\n\n'),
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
    // ENG-FORGE-V4-10C: resolve the exact Smith candidate this Assay lane was
    // meant to verify from existing run evidence so Assay terminal
    // normalization can require a verified workspace base == candidate (and
    // never normalize a wrong-base/failed verification to Complete).
    let context: AssayFinishContext | undefined
    if (item && isAssayTerminalRole(item.role)) {
      const runs = await listStoryRuns(item.storyId, q)
      context = { candidateSha: smithCandidateSha(runs) }
    }
    const normalized = normalizeAgentFinishForRole(item?.role ?? null, input, context)
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
