// ---------------------------------------------------------------------------
// TUnitAgentRuntimeAdapter — the deterministic reference implementation of the
// AgentRuntimeAdapter contract (ENG-18).
//
// This is NOT a mock that bypasses lifecycle behavior: it implements the same
// public operations and protected vendor hooks every real adapter must satisfy,
// and it drives the shared persistence/lifecycle orchestration through the
// injected repositories. Fully deterministic so the adapter contract suite can
// prove success, failure, pause/resume, cancel, retry, heartbeat, evidence
// immutability and terminal-state protection against real Postgres.
//
// No git/fs/vendor operations are performed — this adapter simulates a runtime
// by executing an injected step script.
// ---------------------------------------------------------------------------

import {
  AgentRuntimeAdapter,
  type AdapterStatus,
  type ExternalStatusResult,
  type ExternalStartResult,
} from './agent-runtime-adapter'
import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from './types'
import { CORE_CAPABILITIES, type AgentCapability } from './capabilities'

export type TUnitScriptStep = {
  lifecycle: 'running' | 'paused'
  note?: string
  completion?: number
}

export type TUnitResult = {
  resultStatus: string
  completion: number
  notes: string
  testsSummary: string | null
  commitHash: string | null
}

export type TUnitScenario =
  | { mode: 'success'; steps: TUnitScriptStep[]; result: TUnitResult }
  | { mode: 'failure'; steps: TUnitScriptStep[]; error: string }
  | { mode: 'cancel'; steps: TUnitScriptStep[]; reason: string }

export class TUnitAgentRuntimeAdapter extends AgentRuntimeAdapter {
  readonly runtimeAdapterId = 'tunit'
  readonly capabilities: AgentCapability[] = CORE_CAPABILITIES

  private stepIndex = 0
  private externalRunId: string | null = null
  private liveLifecycle: 'running' | 'paused' | 'cancelled' = 'running'
  private currentStep: string | null = null

  constructor(
    deps: ConstructorParameters<typeof AgentRuntimeAdapter>[0],
    private readonly scenario: TUnitScenario,
    private readonly seedExternalRunId = () =>
      `tunit-run-${Math.random().toString(36).slice(2, 10)}`,
  ) {
    super(deps)
  }


  // -------------------------------------------------------------------------
  // PUBLIC OPERATIONS (vendor hooks + shared orchestration)
  // -------------------------------------------------------------------------

  override async execute(
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
    // (atomic, single-worker-guarded); if already Claimed, proceed. This is
    // shared orchestration — concrete adapters never re-implement it.
    if (command.state === 'Ready') {
      const claimed = await this.deps.work.claimSpecific(command.workItemId, 'tunit')
      if (!claimed) {
        throw new Error(
          `command ${command.workItemId} could not be claimed (another item active or state changed)`,
        )
      }
      command = { ...command, state: 'Claimed', claimedBy: 'tunit' }
    }

    // Shared orchestration: begin the authoritative Story Board run (story ->
    // In Progress, run created with spec snapshot, work item -> Running with
    // story_run_id linked). The adapter then drives its vendor hooks.
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
      note: `run started via tunit adapter (external ${this.externalRunId})`,
    })

    // Drive the deterministic script.
    let status = await this.statusExternal(command, ctxWithRun)
    while (status.lifecycle === 'running') {
      await this.deps.work.progress(command.workItemId, {
        step: this.currentStep ?? 'executing',
        completion: this.stepCompletion(),
        note: this.currentStepNote() ?? undefined,
      })
      status = await this.statusExternal(command, ctxWithRun)
    }

    if (this.scenario.mode === 'failure') {
      await this.deps.work.fail(
        command.workItemId,
        `tunit simulated failure: ${this.scenario.error}`,
      )
      const item = await this.deps.work.get(command.workItemId)
      return this.normalizeEvidenceFromRun(item!.storyRunId!, command)
    }

    if (this.scenario.mode === 'cancel') {
      await this.deps.work.cancel(command.workItemId, this.scenario.reason)
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
  override async status(command: AgentWorkCommand): Promise<AdapterStatus> {
    const detail = {
      step: this.currentStep,
      stepIndex: this.stepIndex,
      externalRunId: this.externalRunId,
    }
    if (command.state === 'Done') return { lifecycle: 'success', externalRunId: this.externalRunId, detail }
    if (command.state === 'Error') return { lifecycle: 'failed', externalRunId: this.externalRunId, detail }
    if (command.state === 'Cancelled') return { lifecycle: 'cancelled', externalRunId: this.externalRunId, detail }
    if (command.state === 'Paused') return { lifecycle: 'paused', externalRunId: this.externalRunId, detail }
    if (command.state === 'Running' || command.state === 'Claimed') {
      return { lifecycle: 'running', externalRunId: this.externalRunId, detail }
    }
    return { lifecycle: 'not_started', externalRunId: null, detail }
  }

  override async pause(command: AgentWorkCommand): Promise<void> {
    this.assertNotTerminal(command)
    this.liveLifecycle = 'paused'
    await this.deps.work.progress(command.workItemId, {
      step: 'paused',
      note: 'tunit runtime paused',
    })
  }

  override async resume(command: AgentWorkCommand): Promise<void> {
    this.assertNotTerminal(command)
    this.liveLifecycle = 'running'
    await this.deps.work.progress(command.workItemId, {
      step: 'executing',
      note: 'tunit runtime resumed',
    })
  }

  override async cancel(command: AgentWorkCommand): Promise<void> {
    this.assertNotTerminal(command)
    this.liveLifecycle = 'running'
    await this.deps.work.cancel(command.workItemId, 'tunit runtime cancelled')
  }

  override async result(
    command: AgentWorkCommand,
  ): Promise<AgentRunEvidence | null> {
    // A result may only be treated as successful after the work item is
    // terminal Done; otherwise the run is not yet a completed success.
    const item = await this.deps.work.get(command.workItemId)
    if (!item?.storyRunId) return null
    const runs = await this.deps.runs.listForStory(item.storyId)
    const run = runs.find((r) => r.id === item.storyRunId) ?? null
    if (!run) return null
    if (item.state !== 'Done' && item.state !== 'Error' && item.state !== 'Cancelled') {
      // Non-terminal: never report a successful evidence record.
      return null
    }
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

  // -------------------------------------------------------------------------
  // PROTECTED VENDOR HOOKS
  // -------------------------------------------------------------------------

  protected async startExternal(
    _context: AgentExecutionContext,
  ): Promise<ExternalStartResult> {
    this.externalRunId = this.seedExternalRunId()
    return { externalRunId: this.externalRunId }
  }


  protected async statusExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<ExternalStatusResult> {
    const steps = this.scenario.steps
    if (this.stepIndex >= steps.length) {
      // Script exhausted — determine terminal lifecycle from mode.
      if (this.scenario.mode === 'failure') return { lifecycle: 'failed' }
      if (this.scenario.mode === 'cancel') return { lifecycle: 'cancelled' }
      return { lifecycle: 'success' }
    }
    const step = steps[this.stepIndex]
    this.stepIndex += 1
    this.currentStep = step.note ?? null
    return { lifecycle: step.lifecycle }
  }

  protected async pauseExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    this.liveLifecycle = 'paused'
  }

  protected async resumeExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    this.liveLifecycle = 'running'
  }

  protected async cancelExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    this.liveLifecycle = 'cancelled'
  }

  protected async resultExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null> {
    if (this.scenario.mode !== 'success') return null
    return {
      resultStatus: this.scenario.result.resultStatus,
      completion: this.scenario.result.completion,
      notes: this.scenario.result.notes,
      testsSummary: this.scenario.result.testsSummary,
      commitHash: this.scenario.result.commitHash,
      runtimeAdapter: this.runtimeAdapterId,
      modelProfile: null,
      externalRunId: this.externalRunId,
      startedAt: new Date().toISOString(),
      endedAt: new Date().toISOString(),
    }
  }
  // -------------------------------------------------------------------------
  // Helpers
  // -------------------------------------------------------------------------

  private stepCompletion(): number {
    const steps = this.scenario.steps
    const last = steps[Math.max(0, this.stepIndex - 1)]
    return last?.completion ?? 10
  }

  private currentStepNote(): string | null {
    const steps = this.scenario.steps
    const last = steps[Math.max(0, this.stepIndex - 1)]
    return last?.note ?? null
  }

  private normalizeEvidence(
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

  private async normalizeEvidenceFromRun(
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
