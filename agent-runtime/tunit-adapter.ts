// ---------------------------------------------------------------------------
// TUnitAgentRuntimeAdapter — the deterministic reference receiver for the
// AgentRuntimeAdapter contract (ENG-18 / ENG-19).
//
// After the shared-lifecycle extraction (ENG-19 Slice A), this class contains
// ONLY vendor-hook behavior + deterministic test scripting. The base
// AgentRuntimeAdapter owns all shared orchestration/persistence (execute,
// claim, begin-run, heartbeat loop, terminal normalization, evidence).
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
  // PROTECTED VENDOR HOOKS ONLY (shared lifecycle lives in the base class)
  // -------------------------------------------------------------------------

  protected async startExternal(
    _context: AgentExecutionContext,
  ): Promise<ExternalStartResult> {
    this.externalRunId = this.seedExternalRunId()
    if (this.scenario.mode === 'failure') {
      this.externalErrorText = `tunit simulated failure: ${this.scenario.error}`
    }
    if (this.scenario.mode === 'cancel') {
      this.externalErrorText = `tunit simulated cancellation: ${this.scenario.reason}`
    }
    return { externalRunId: this.externalRunId }
  }

  protected async statusExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<ExternalStatusResult> {
    const steps = this.scenario.steps
    if (this.stepIndex >= steps.length) {
      // Script exhausted — terminal lifecycle depends on the scenario mode.
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
    // Deterministic reference: no external runtime to pause; the base persists
    // the durable pause heartbeat.
  }

  protected async resumeExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    // Deterministic reference: resume is the same logical attempt.
  }

  protected async cancelExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    // The base persists canonical cancellation.
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
  // Shared helpers the base does not provide (scripted step projection)
  // -------------------------------------------------------------------------

  protected override progressFromStatus(
    _status: ExternalStatusResult,
    _command: AgentWorkCommand,
  ): { step?: string; completion?: number; note?: string } | null {
    const steps = this.scenario.steps
    const last = steps[Math.max(0, this.stepIndex - 1)]
    return {
      step: this.currentStep ?? 'executing',
      completion: last?.completion ?? 10,
      note: this.currentStep ?? undefined,
    }
  }

  // The base `status()` reads persisted canonical state; keep the derived
  // lifecycle mapping for backward compatibility with the contract suite.
  override async status(command: AgentWorkCommand): Promise<AdapterStatus> {
    return super.status(command)
  }
}
