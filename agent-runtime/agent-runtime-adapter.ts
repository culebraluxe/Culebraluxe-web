// ---------------------------------------------------------------------------
// Abstract AgentRuntimeAdapter — the CulebraLuxe-owned execution contract.
//
// PUBLIC / STABLE operations (shared lifecycle behavior lives here):
//   execute, status, pause, resume, cancel, result
//
// VENDOR-SPECIFIC behavior lives ONLY in the protected abstract hooks:
//   startExternal, statusExternal, pauseExternal, resumeExternal,
//   cancelExternal, resultExternal
//
// The base class owns shared orchestration + persistence through injected
// repositories/services — concrete adapters never emit raw SQL. No vendor
// nouns may appear anywhere in this contract.
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

  constructor(protected readonly deps: AgentRuntimeAdapterDeps) {}

  // -------------------------------------------------------------------------
  // PUBLIC / STABLE OPERATIONS
  // -------------------------------------------------------------------------

  /**
   * Execute one AgentWorkCommand as one logical run. Shared orchestration:
   *   1. idempotency guard — a terminal command cannot be executed again
   *   2. create/lookup the Story Board run (authoritative evidence)
   *   3. call startExternal
   *   4. persist running state + heartbeat
   *   5. call the vendor result hook
   *   6. normalize + persist evidence
   * Subclasses provide only the vendor hooks.
   */
  abstract execute(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence>

  /** Query runtime status of an external run/session. */
  abstract status(command: AgentWorkCommand): Promise<AdapterStatus>

  /** Pause an active run (preserves assignment/context). */
  abstract pause(command: AgentWorkCommand): Promise<void>

  /** Resume a paused run (same logical attempt). */
  abstract resume(command: AgentWorkCommand): Promise<void>

  /** Cancel an active run (terminal, never success). */
  abstract cancel(command: AgentWorkCommand): Promise<void>

  /** Retrieve normalized evidence for a run. */
  abstract result(command: AgentWorkCommand): Promise<AgentRunEvidence | null>

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

  /** Shared terminal-state guard. */
  protected assertNotTerminal(command: AgentWorkCommand): void {
    if (command.state === 'Done' || command.state === 'Error' || command.state === 'Cancelled') {
      throw new Error(
        `command ${command.workItemId} is already terminal (${command.state})`,
      )
    }
  }
}
