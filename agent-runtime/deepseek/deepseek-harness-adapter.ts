// ---------------------------------------------------------------------------
// DeepSeekHarnessAdapter — thin translation layer from the vendor-neutral
// AgentRuntimeAdapter contract to the DeepSeek Harness headless CLI (ENG-19).
//
// The base class owns ALL shared lifecycle orchestration (claim, begin-run,
// heartbeat loop, terminal normalization, evidence persistence). This class
// implements ONLY the protected vendor hooks + a tiny process wrapper
// (agent-runtime/deepseek/dsh-client).
//
// NO DeepSeek nouns leak upward: the command model, Story Board, invoker,
// canonical statuses, and logical model-profile vocabulary stay generic.
// `external_run_id` = the opaque `session-<uuid>` directory name discovered
// after the run. Canonical state remains in CulebraLuxe tables.
// ---------------------------------------------------------------------------

import {
  AgentRuntimeAdapter,
  type ExternalStartResult,
  type ExternalStatusResult,
} from '../agent-runtime-adapter'
import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from '../types'
import type { AgentCapability } from '../capabilities'
import {
  startDshRun,
  type DshHandle,
  type DshRunResult,
} from './dsh-client'

const DEEPSEEK_CAPABILITIES: AgentCapability[] = [
  'workspace.fs.read',
  'workspace.fs.write',
  'workspace.fs.delete',
  'git.status',
  'git.diff',
  'git.history',
  'git.commit',
  'host.exec',
  'host.tests',
  'host.typecheck',
  'host.build',
  'host.repo-scripts',
]

export type DeepSeekHarnessConfig = {
  /** Absolute path to the dsh CLI bin (e.g. $DSH_HOME/profiles/node_modules/@deepseek-ai/dsh/lib/bin.js). */
  cliBin: string
  /** Repository workspace the harness operates in. */
  workspace: string
  /** Optional env overrides (API key is read from process env by the harness). */
  env?: Record<string, string | undefined>
  /** Injectable start function (tests substitute a fake handle). */
  startRun?: (opts: {
    cliBin: string
    cwd: string
    task: string
    env?: Record<string, string | undefined>
  }) => DshHandle
}

/** Build the agent task text from canonical story context (below the boundary). */
export function buildTaskText(
  command: AgentWorkCommand,
  context: AgentExecutionContext,
): string {
  const s = context.story
  const parts: string[] = []
  parts.push(`Execute SDLC story ${s.id}: ${s.title}.`)
  if (s.goal) parts.push(`Goal: ${s.goal}`)
  if (context.command.specialInstructions) {
    parts.push(`Special instructions (additive, do not replace the architect brief): ${context.command.specialInstructions}`)
  }
  if (s.architectBrief) parts.push(`Architect brief: ${s.architectBrief}`)
  if (s.acceptanceCriteria) parts.push(`Acceptance criteria (do not mark Complete unless these are satisfied): ${s.acceptanceCriteria}`)
  parts.push(
    'Work in the current repository. Verify your work by running tests/typecheck/build. ' +
    'Create a local git commit with the intended changes when the story requires it. ' +
    'Do NOT push. Do NOT mutate production data or schema. Report what you did.',
  )
  return parts.join('\n')
}

export class DeepSeekHarnessAdapter extends AgentRuntimeAdapter {
  readonly runtimeAdapterId = 'deepseek-harness'
  readonly capabilities: AgentCapability[] = DEEPSEEK_CAPABILITIES

  private handle: DshHandle | null = null
  private lastResult: DshRunResult | null = null

  constructor(
    deps: ConstructorParameters<typeof AgentRuntimeAdapter>[0],
    private readonly config: DeepSeekHarnessConfig,
    private readonly taskBuilder: (
      command: AgentWorkCommand,
      context: AgentExecutionContext,
    ) => string = buildTaskText,
  ) {
    super(deps)
  }

  // -------------------------------------------------------------------------
  // PROTECTED VENDOR HOOKS (shared lifecycle lives in the base class)
  // -------------------------------------------------------------------------

  protected async startExternal(
    context: AgentExecutionContext,
  ): Promise<ExternalStartResult> {
    const task = this.taskBuilder(context.command, context)
    const startRun = this.config.startRun ?? startDshRun
    this.handle = startRun({
      cliBin: this.config.cliBin,
      cwd: this.config.workspace,
      task,
      env: this.config.env,
    })

    // Opaque correlation: the harness generates session-<uuid> internally and
    // does NOT print it. The client discovers it after completion. Until then
    // we use a provisional id; resultExternal updates it to the real session.
    this.externalRunId = `deepseek-pending-${Date.now()}`
    return { externalRunId: this.externalRunId }
  }

  protected async statusExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<ExternalStatusResult> {
    if (!this.handle) return { lifecycle: 'failed' }
    // Cancellation is requested first: SIGTERM sent. The child may not have
    // exited yet; map directly to the canonical cancelled lifecycle so the
    // shared loop terminalizes without racing the process exit.
    if (this.handle.cancelled) return { lifecycle: 'cancelled' }
    if (this.handle.proc.exitCode === null) {
      return { lifecycle: 'running', detail: { sessionId: this.externalRunId } }
    }
    // Process exited — resolve the final result exactly once.
    if (!this.lastResult) {
      this.lastResult = await this.handle.promise
    }
    if (this.lastResult.status === 'failed' && !this.externalErrorText) {
      this.externalErrorText =
        this.lastResult.stderr.trim() ||
        `dsh exited ${this.lastResult.exitCode ?? 'no-code'}`
    }
    if (this.lastResult.status === 'success') return { lifecycle: 'success' }
    return { lifecycle: 'failed' }
  }

  protected async pauseExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    if (!this.handle) return
    // Semantics-preserving process pause (SIGSTOP). The harness has no native
    // headless pause; the child + its session survive the suspension.
    this.handle.pause()
  }

  protected async resumeExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    if (!this.handle) return
    this.handle.resume()
  }

  protected async cancelExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    if (!this.handle) return
    this.handle.cancel()
  }

  protected async resultExternal(
    command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null> {
    if (!this.handle) return null
    const result = await this.handle.promise
    this.lastResult = result

    // Update opaque correlation to the real harness session id when found.
    if (result.sessionId) {
      this.externalRunId = result.sessionId
      try {
        await this.deps.work.setRuntime(command.workItemId, {
          runtimeAdapter: this.runtimeAdapterId,
          externalRunId: result.sessionId,
        })
      } catch {
        // Evidence still records the correlation even if the work-item update
        // races a terminal transition.
      }
    }

    if (result.status === 'failed') {
      this.externalErrorText =
        result.stderr.trim() || `dsh exited ${result.exitCode ?? 'no-code'}`
      return null
    }

    const notes = [
      'DeepSeek Harness run completed.',
      result.stdout.trim() ? `Assistant output:\n${result.stdout.trim()}` : 'No assistant text captured.',
      result.stderr.trim() ? `stderr:\n${result.stderr.trim()}` : null,
      result.sessionDir ? `Session transcript: ${result.sessionDir}` : null,
    ]
      .filter(Boolean)
      .join('\n\n')

    return {
      resultStatus: 'Complete',
      completion: 100,
      notes,
      testsSummary: `dsh exit code ${result.exitCode}`,
      commitHash: null,
      runtimeAdapter: this.runtimeAdapterId,
      modelProfile: command.modelProfile,
      externalRunId: this.externalRunId,
      startedAt: new Date().toISOString(),
      endedAt: new Date().toISOString(),
    }
  }

  // -------------------------------------------------------------------------
  // Progress projection: the harness exposes no mid-run percent; report a
  // factual liveness step only.
  // -------------------------------------------------------------------------

  protected override progressFromStatus(
    _status: ExternalStatusResult,
    _command: AgentWorkCommand,
  ): { step?: string; completion?: number; note?: string } | null {
    return {
      step: 'executing',
      note: `deepseek harness running (external ${this.externalRunId})`,
    }
  }
}


