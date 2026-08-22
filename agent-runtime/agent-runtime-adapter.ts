// ---------------------------------------------------------------------------
// Abstract AgentRuntimeAdapter — the CulebraLuxe-owned execution contract.
//
// PUBLIC / STABLE operations (SHARED lifecycle behavior lives here):
//   execute, status, pause, resume, cancel, result
//
// VENDOR-SPECIFIC behavior lives ONLY in the protected abstract hooks:
//   startExternal, statusExternal, pauseExternal, resumeExternal,
//   cancelExternal, resultExternal
//
// The base class OWNS common orchestration + persistence through injected
// repositories/services — concrete adapters never emit raw SQL and never
// re-implement the shared lifecycle. No vendor nouns may appear anywhere in
// this contract.
// ---------------------------------------------------------------------------

import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from './types'
import type {
  AgentRunRepository,
  AgentWorkRepository,
} from './repositories'
import type { AgentCapability } from './capabilities'

export type AdapterLifecycle =
  | 'not_started'
  | 'starting'
  | 'running'
  | 'paused'
  | 'success'
  | 'failed'
  | 'cancelled'

export type AdapterStatus = {
  lifecycle: AdapterLifecycle
  externalRunId: string | null
  detail: Record<string, unknown>
}

export interface AgentRuntimeAdapterDeps {
  work: AgentWorkRepository
  runs: AgentRunRepository
  /** Clock injection for deterministic tests. */
  now?: () => Date
}

/** External start outcome (opaque correlation id only). */
export type ExternalStartResult = {
  externalRunId: string
}

export type ExternalStatusResult = {
  lifecycle: AdapterLifecycle
  detail?: Record<string, unknown>
}

export abstract class AgentRuntimeAdapter {
  /** Stable adapter identity (e.g. 'tunit', 'deepseek-harness', 'local-mac'). */
  abstract readonly runtimeAdapterId: string

  /** Capabilities this adapter can satisfy. */
  abstract readonly capabilities: AgentCapability[]

  /** Opaque external run/session correlation for the current attempt. */
  protected externalRunId: string | null = null

  /** Optional external error detail captured by the vendor hooks. */
  protected externalErrorText: string | null = null

  /** Story Board spec of the most recent execute (used by pause/resume/cancel hooks). */
  protected currentStory: AgentExecutionContext['story'] | null = null

  constructor(protected readonly deps: AgentRuntimeAdapterDeps) {}

  // -------------------------------------------------------------------------
  // PUBLIC / STABLE OPERATIONS (shared orchestration — do not override)
  // -------------------------------------------------------------------------

  /**
   * Execute one AgentWorkCommand as one logical run. SHARED orchestration:
   *   1. terminal/idempotency guard using the PERSISTED command state
   *   2. claim-if-Ready (atomic, single-worker-guarded)
   *   3. begin the authoritative Story Board run (work -> Running, spec snapshot)
   *   4. startExternal -> opaque externalRunId persisted on the work item
   *   5. heartbeat/progress loop over statusExternal until terminal
   *   6. normalize result: fail / cancel / finish according to terminal state
   * Concrete adapters provide ONLY the vendor hooks.
   */
  async execute(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence> {
    this.assertNotTerminal(command)

    // Re-read the durable command so the terminal/active decision uses the
    // PERSISTED state, not a possibly-stale in-memory copy.
    const persisted = await this.deps.work.get(command.workItemId)
    if (!persisted) {
      throw new Error(`command ${command.workItemId} no longer exists`)
    }
    if (persisted.state === 'Done' || persisted.state === 'Error' || persisted.state === 'Cancelled') {
      throw new Error(`command ${command.workItemId} is already terminal (${persisted.state})`)
    }
    command = { ...command, state: persisted.state as AgentWorkCommand['state'] }

    // Invoker-style claim: if the durable command is still Ready, claim it
    // (atomic, single-worker-guarded); if already Claimed, proceed.
    if (command.state === 'Ready') {
      const claimed = await this.deps.work.claimSpecific(command.workItemId, this.runtimeAdapterId)
      if (!claimed) {
        throw new Error(
          `command ${command.workItemId} could not be claimed (another item active or state changed)`,
        )
      }
      command = { ...command, state: 'Claimed', claimedBy: this.runtimeAdapterId }
    }

    // Begin the authoritative Story Board run (story -> In Progress, run
    // created with spec snapshot, work item -> Running with story_run_id).
    this.currentStory = context.story
    const begun = await this.deps.work.beginRun(command.workItemId)
    const runId = begun.workItem.storyRunId!
    const ctxWithRun = { ...context, storyRunId: runId }

    const started = await this.startExternal(ctxWithRun)
    this.externalRunId = started.externalRunId
    await this.deps.work.setRuntime(command.workItemId, {
      runtimeAdapter: this.runtimeAdapterId,
      externalRunId: this.externalRunId,
    })

    // Persist running + heartbeat.
    await this.deps.work.progress(command.workItemId, {
      step: 'executing',
      completion: 10,
      note: `run started via ${this.runtimeAdapterId} (external ${this.externalRunId})`,
    })


    // Heartbeat/progress loop until the vendor runtime reaches a terminal
    // lifecycle. Every pass refreshes liveness (heartbeat) via the work repo.
    let status = await this.statusExternal(command, ctxWithRun)
    while (status.lifecycle === 'running') {
      const prog = this.progressFromStatus(status, command)
      await this.deps.work.progress(command.workItemId, {
        step: prog?.step,
        completion: prog?.completion,
        note: prog?.note,
      })
      status = await this.statusExternal(command, ctxWithRun)
    }

    if (status.lifecycle === 'failed') {
      await this.deps.work.fail(
        command.workItemId,
        this.externalErrorText ?? 'runtime failed',
      )
      const item = await this.deps.work.get(command.workItemId)
      return this.normalizeEvidenceFromRun(item!.storyRunId!, command)
    }

    if (status.lifecycle === 'cancelled') {
      await this.deps.work.cancel(command.workItemId, this.externalErrorText ?? 'runtime cancelled')
      const item = await this.deps.work.get(command.workItemId)
      return this.normalizeEvidenceFromRun(item!.storyRunId!, command)
    }

    // Success path.
    const result = await this.resultExternal(command, ctxWithRun)
    const finished = await this.deps.work.finish(command.workItemId, {
      resultStatus: result!.resultStatus,
      completion: result!.completion,
      notes: result!.notes,
      commitHash: result!.commitHash,
      testsSummary: result!.testsSummary,
    })
    return this.normalizeEvidence(finished.run as any, command)
  }

  /** Query runtime status from persisted canonical state + optional vendor detail. */
  async status(command: AgentWorkCommand): Promise<AdapterStatus> {
    const item = await this.deps.work.get(command.workItemId)
    const state = item?.state ?? command.state
    const runId = this.externalRunId ?? item?.externalRunId ?? null
    const detail: Record<string, unknown> = { externalRunId: runId }
    if (state === 'Done') return { lifecycle: 'success', externalRunId: runId, detail }
    if (state === 'Error') return { lifecycle: 'failed', externalRunId: runId, detail }
    if (state === 'Cancelled') return { lifecycle: 'cancelled', externalRunId: runId, detail }
    if (state === 'Paused') return { lifecycle: 'paused', externalRunId: runId, detail }
    if (state === 'Running' || state === 'Claimed') {
      return { lifecycle: 'running', externalRunId: runId, detail }
    }
    return { lifecycle: 'not_started', externalRunId: null, detail }
  }

  /** Pause an active run: vendor hook + durable heartbeat (assignment preserved). */
  async pause(command: AgentWorkCommand): Promise<void> {
    this.assertNotTerminal(command)
    await this.pauseExternal(command, this.contextWithRun(command))
    await this.deps.work.progress(command.workItemId, {
      step: 'paused',
      note: `${this.runtimeAdapterId} runtime paused`,
    })
  }

  /** Resume a paused run: vendor hook + durable heartbeat (same logical attempt). */
  async resume(command: AgentWorkCommand): Promise<void> {
    this.assertNotTerminal(command)
    await this.resumeExternal(command, this.contextWithRun(command))
    await this.deps.work.progress(command.workItemId, {
      step: 'executing',
      note: `${this.runtimeAdapterId} runtime resumed`,
    })
  }

  /** Cancel an active run: vendor hook + canonical cancellation (never success). */
  async cancel(command: AgentWorkCommand): Promise<void> {
    this.assertNotTerminal(command)
    await this.cancelExternal(command, this.contextWithRun(command))
    await this.deps.work.cancel(command.workItemId, `${this.runtimeAdapterId} runtime cancelled`)
  }

  /**
   * Retrieve normalized evidence. A result may only be treated as successful
   * after the work item is terminal; otherwise the run is not complete.
   */
  async result(command: AgentWorkCommand): Promise<AgentRunEvidence | null> {
    const item = await this.deps.work.get(command.workItemId)
    if (!item?.storyRunId) return null
    const runs = await this.deps.runs.listForStory(item.storyId)
    const run = runs.find((r) => r.id === item.storyRunId) ?? null
    if (!run) return null
    if (item.state !== 'Done' && item.state !== 'Error' && item.state !== 'Cancelled') {
      return null
    }
    return this.normalizeEvidence(run as any, command)
  }


  // -------------------------------------------------------------------------
  // PROTECTED VENDOR HOOKS — the ONLY place concrete adapters translate to a
  // specific harness/model. No shared lifecycle logic belongs here.
  // -------------------------------------------------------------------------

  protected abstract startExternal(
    context: AgentExecutionContext,
  ): Promise<ExternalStartResult>

  protected abstract statusExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<ExternalStatusResult>

  protected abstract pauseExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<void>

  protected abstract resumeExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<void>

  protected abstract cancelExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<void>

  protected abstract resultExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null>

  // -------------------------------------------------------------------------
  // SHARED helpers
  // -------------------------------------------------------------------------

  /** Shared terminal-state guard. */
  protected assertNotTerminal(command: AgentWorkCommand): void {
    if (command.state === 'Done' || command.state === 'Error' || command.state === 'Cancelled') {
      throw new Error(
        `command ${command.workItemId} is already terminal (${command.state})`,
      )
    }
  }

  /** Optional per-status progress projection (concrete adapters override to
   * expose factual step/completion/note from a status poll). */
  protected progressFromStatus(
    _status: ExternalStatusResult,
    _command: AgentWorkCommand,
  ): { step?: string; completion?: number; note?: string } | null {
    return null
  }

  private contextWithRun(command: AgentWorkCommand): AgentExecutionContext {
    // A minimal context for hooks that do not need the full Story Board spec.
    return {
      command,
      story: this.currentStory as never,
      policy: { allowCommit: false, allowDevDbWrite: false, allowControlPlaneWrite: true },
      capabilities: this.capabilities,
      storyRunId: command.storyRunId ?? '',
    }
  }

  protected normalizeEvidence(
    run: {
      id: string
      storyId: string
      startedAt: string
      endedAt: string | null
      resultStatus: string | null
      completion: number | null
      notes: string | null
      testsSummary: string | null
      commitHash: string | null
    },
    command: AgentWorkCommand,
  ): AgentRunEvidence {
    return {
      resultStatus: run.resultStatus ?? 'Failed',
      completion: run.completion ?? 0,
      notes: run.notes ?? '',
      testsSummary: run.testsSummary ?? null,
      commitHash: run.commitHash ?? null,
      runtimeAdapter: this.runtimeAdapterId,
      modelProfile: command.modelProfile,
      externalRunId: this.externalRunId,
      startedAt: run.startedAt,
      endedAt: run.endedAt,
    }
  }

  protected async normalizeEvidenceFromRun(
    runId: string,
    command: AgentWorkCommand,
  ): Promise<AgentRunEvidence> {
    const runs = await this.deps.runs.listForStory(command.storyId)
    const run = runs.find((r) => r.id === runId)
    if (!run) {
      return {
        resultStatus: 'Failed',
        completion: 0,
        notes: 'run not found after terminal transition',
        testsSummary: null,
        commitHash: null,
        runtimeAdapter: this.runtimeAdapterId,
        modelProfile: command.modelProfile,
        externalRunId: this.externalRunId,
        startedAt: new Date().toISOString(),
        endedAt: new Date().toISOString(),
      }
    }
    return this.normalizeEvidence(run as any, command)
  }
}
