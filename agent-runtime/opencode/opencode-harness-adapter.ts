// ---------------------------------------------------------------------------
// OpenCodeHarnessAdapter — thin translation layer from the vendor-neutral
// AgentRuntimeAdapter contract to the OpenCode CLI (`opencode run`) as an
// INNER Smith execution harness (ENG-FORGE-V5-01).
//
// OpenCode is an inner execution engine, NOT a second orchestrator. Forge
// stays outside it and owns the lifecycle:
//
//   Forge worktree -> OpenCode Smith execution -> Forge candidate commit
//     -> exact-candidate Assay -> Forge publish
//
// The base class (AgentRuntimeAdapter) owns ALL shared lifecycle orchestration
// (claim, begin-run, heartbeat loop, terminal normalization, evidence
// persistence). This class implements ONLY the protected vendor hooks plus a
// tiny process wrapper (agent-runtime/opencode/opencode-client).
//
// Contract invariants (deliberately boring, fail closed):
//   - The adapter runs ONLY inside the Forge-provisioned isolated worker
//     worktree (context.executionWorkspace). It refuses to start in an
//     unisolated/shared checkout: `--auto` must never leave the Forge worktree.
//   - The model is ALWAYS passed explicitly as `deepseek/deepseek-v4-flash`
//     (OPENCODE_PINNED_MODEL). A missing or different explicit model is a
//     fail-closed error — OpenCode's default/automatic model selection is
//     never allowed, because it would invalidate A/B measurements.
//   - The canonical Forge task/prompt contract (buildTaskText) is preserved
//     verbatim as the message passed to `opencode run`.
//   - OpenCode never owns git branch creation, candidate commit creation,
//     Assay, publish, story state, or Neon state: Forge provisions the branch
//     and worktree, reads/creates the candidate commit through the existing
//     harness-owned commit path, and owns Assay + publish unchanged.
//   - No OpenCode server mode, ACP, MCP, subagents, sessions, TUI automation,
//     swarm, or provider orchestration is added here.
//
// NO OpenCode nouns leak upward: the command model, Story Board, invoker, and
// canonical statuses stay generic. Evidence records factual run metadata
// (harness=opencode, model, worktree/cwd, exit status, elapsed ms) but no
// activity-based costing is built.
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
  buildTaskText,
  extractTestsSummary,
  workspaceEvidenceLine,
} from '../deepseek/deepseek-harness-adapter'
import {
  startOpenCodeRun,
  type OpenCodeHandle,
  type OpenCodeRunResult,
} from './opencode-client'
import {
  detectFullRegressionAttempt,
  resolveTestModeFromInstructions,
} from '../test-mode'
import {
  assertExecutionTargetSafe,
  buildChildProcessEnv,
  verifyWorkspaceEnvFile,
} from '../../lib/execution-target'
import { readWorkerCommitHash } from '../../lib/worker-workspace'
import { resolve } from 'node:path'

/**
 * The single model this adapter is allowed to pin. ENG-FORGE-V5-01 pins the
 * first supported model explicitly; a different value is a fail-closed error
 * so a run can never silently execute under OpenCode's default model
 * selection (which would invalidate A/B measurements against forge-native).
 */
export const OPENCODE_PINNED_MODEL = 'deepseek/deepseek-v4-flash'

const OPENCODE_CAPABILITIES: AgentCapability[] = [
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

export type OpenCodeHarnessConfig = {
  /** Executable to run: `opencode` (PATH) or an absolute entrypoint. */
  cliBin: string
  /** Repository workspace the harness operates in (unused when the invoker
   *  provisions an isolated worker worktree). */
  workspace: string
  /** Explicit model id. Defaults to OPENCODE_PINNED_MODEL; anything blank or
   *  different fails closed. */
  model?: string
  /** Optional env overrides (API keys are read from process env by OpenCode). */
  env?: Record<string, string | undefined>
  /** Injectable start function (tests substitute a fake handle). */
  startRun?: (opts: {
    cliBin: string
    cwd: string
    model: string
    task: string
    env?: Record<string, string | undefined>
  }) => OpenCodeHandle
}

/** Default OpenCode harness configuration for the local host. */
export function defaultOpenCodeConfig(
  env: NodeJS.ProcessEnv = process.env,
): OpenCodeHarnessConfig {
  return {
    cliBin: env.OPENCODE_BIN?.trim() || 'opencode',
    workspace: resolve(process.cwd()),
    model: OPENCODE_PINNED_MODEL,
  }
}

/**
 * Fail-closed model resolution. An omitted model resolves to the
 * ENG-FORGE-V5-01 pin (the adapter ALWAYS passes --model; OpenCode's own
 * default/automatic model selection is never exercised). An explicitly blank
 * model or any model other than the pin throws with a truthful reason — the
 * adapter never falls back to another model or to OpenCode's default.
 */
export function resolveOpenCodeModel(
  model: string | null | undefined,
): string {
  // An OMITTED model resolves to the pin: the adapter always passes an
  // explicit --model and never leaves model selection to OpenCode.
  if (model === null || model === undefined) return OPENCODE_PINNED_MODEL
  const value = model.trim()
  if (!value) {
    throw new Error(
      `OpenCode harness has no explicit model configuration: expected '${OPENCODE_PINNED_MODEL}', got empty. Refusing to rely on OpenCode's default model selection.`,
    )
  }
  if (value !== OPENCODE_PINNED_MODEL) {
    throw new Error(
      `OpenCode harness model '${value}' is not the ENG-FORGE-V5-01 pinned model '${OPENCODE_PINNED_MODEL}'. No fallback model or harness substitution is allowed.`,
    )
  }
  return value
}

/** Non-throwing readiness variant: returns a blocker reason or null. */
export function openCodeModelBlocker(
  model: string | null | undefined,
): string | null {
  if (model === null || model === undefined) return null
  const value = model.trim()
  if (!value) {
    return `OpenCode harness has no explicit model configuration: expected '${OPENCODE_PINNED_MODEL}', got empty. Refusing to rely on OpenCode's default model selection.`
  }
  if (value !== OPENCODE_PINNED_MODEL) {
    return `OpenCode harness model '${value}' is not the ENG-FORGE-V5-01 pinned model '${OPENCODE_PINNED_MODEL}'. No fallback model or harness substitution is allowed.`
  }
  return null
}

export class OpenCodeHarnessAdapter extends AgentRuntimeAdapter {
  readonly runtimeAdapterId = 'opencode-harness'
  readonly capabilities: AgentCapability[] = OPENCODE_CAPABILITIES

  private handle: OpenCodeHandle | null = null
  private lastResult: OpenCodeRunResult | null = null
  /** Wall-clock start of the OpenCode process (factual elapsed-time evidence). */
  private startedAtMs: number | null = null

  constructor(
    deps: ConstructorParameters<typeof AgentRuntimeAdapter>[0],
    private readonly config: OpenCodeHarnessConfig,
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
    // Defensive FAIL-FAST (ENG-20/ENG-20A): never spawn external work for an
    // execution target whose application/domain DB configuration would resolve
    // to the production database. Second barrier directly before the harness
    // process is started (the invoker guards before calling execute).
    const target = (context.executionEnvironment ?? 'DEV') as never
    assertExecutionTargetSafe(target)

    // ENG-FORGE-V5-01: OpenCode runs INSIDE the Forge-provisioned isolated
    // worker worktree only. `--auto` (permission auto-approval) must never
    // reach an unisolated/shared checkout, and OpenCode must not choose or
    // create its own workspace. Missing isolation fails closed with a
    // truthful reason — no fallback to the shared checkout.
    const workspace = context.executionWorkspace?.worktreePath
    if (!workspace) {
      throw new Error(
        'opencode-harness requires the Forge-provisioned isolated worker worktree (context.executionWorkspace); refusing to run `opencode run --auto` outside the Forge worktree.',
      )
    }
    verifyWorkspaceEnvFile(workspace, target)

    // Explicit model pinning — never OpenCode's default model selection.
    const model = resolveOpenCodeModel(this.config.model)

    // Canonical Forge task/prompt contract preserved verbatim.
    const task = this.taskBuilder(context.command, context)

    const startRun = this.config.startRun ?? startOpenCodeRun
    // DEV safety: the spawned harness (and any test process it spawns) must
    // NOT inherit an APP_ENV/DATABASE_URL set that resolves to the production
    // application database.
    const childEnv = buildChildProcessEnv(target)
    this.startedAtMs = Date.now()
    this.handle = startRun({
      cliBin: this.config.cliBin,
      cwd: workspace,
      model,
      task,
      env: { ...childEnv, ...(this.config.env ?? {}) },
    })
    this.externalRunId = `opencode-${Date.now()}`
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
    // Still alive / not yet settled -> running. `done` also covers a spawn
    // failure (missing binary), which settles immediately as failed below —
    // the poll can never report a dead launch as running forever.
    if (!this.handle.done) {
      return { lifecycle: 'running', detail: { externalRunId: this.externalRunId } }
    }
    // Process settled — resolve the final result exactly once.
    if (!this.lastResult) {
      this.lastResult = await this.handle.promise
    }
    if (this.lastResult.status === 'failed' && !this.externalErrorText) {
      this.externalErrorText =
        this.lastResult.stderr.trim() ||
        `opencode exited ${this.lastResult.exitCode ?? 'no-code'}`
    }
    if (this.lastResult.status === 'success') return { lifecycle: 'success' }
    return { lifecycle: 'failed' }
  }

  protected async pauseExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    if (!this.handle) return
    // Semantics-preserving process pause (SIGSTOP); the child survives.
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
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null> {
    if (!this.handle) return null
    const result = await this.handle.promise
    this.lastResult = result

    if (result.status === 'failed') {
      this.externalErrorText =
        result.stderr.trim() || `opencode exited ${result.exitCode ?? 'no-code'}`
      return null
    }

    const model = resolveOpenCodeModel(this.config.model)
    const executionWorkspace = context.executionWorkspace
    const workspace = executionWorkspace?.worktreePath
    if (!workspace || !executionWorkspace) {
      // startExternal already refused to run without an isolated workspace;
      // this is defense-in-depth for direct hook misuse.
      return null
    }

    // ENG-FORGE-V5-01 / AC5: Forge, not OpenCode, owns the candidate commit
    // through the existing harness-owned commit path. This hook reads the
    // factual HEAD of the worker worktree; when OpenCode left the workspace
    // dirty (no commit), the outer Forge policy wrapper creates the candidate
    // commit itself via commitWorkerWorkspaceChanges. Null is persisted when
    // the checkout is exactly at the approved base commit — never fabricated.
    const commitHash = await readWorkerCommitHash(
      workspace,
      executionWorkspace.baseCommit,
    )

    const elapsedMs =
      this.startedAtMs !== null ? Date.now() - this.startedAtMs : null

    const notes = [
      'OpenCode run completed.',
      `Run metadata: harness=opencode model=${model} worktree=${workspace} exit=${result.exitCode ?? 'n/a'}${
        elapsedMs !== null ? ` elapsed_ms=${elapsedMs}` : ''
      }`,
      workspaceEvidenceLine(executionWorkspace),
      result.stdout.trim() ? `Assistant output:\n${result.stdout.trim()}` : 'No assistant text captured.',
      result.stderr.trim() ? `stderr:\n${result.stderr.trim()}` : null,
    ]
      .filter(Boolean)
      .join('\n\n')

    // FULL-harness guard (SCOPED mode): if the model reported invoking a known
    // full-regression command, flag it in the durable evidence so the operator
    // sees a policy violation instead of silently accepting the tax.
    const { mode } = resolveTestModeFromInstructions(
      command.specialInstructions,
      process.env.AGENT_TEST_MODE ?? null,
    )
    const forbidden =
      mode === 'SCOPED' ? detectFullRegressionAttempt(result.stdout) : null

    // ENG-08 — concise tests/checks summary: prefer the model's deliberate
    // `Tests: ...` evidence line; fall back to the factual exit code. A SCOPED
    // violation replaces the summary so the operator sees the policy breach.
    const testsSummary = extractTestsSummary(
      result.stdout,
      `opencode exit code ${result.exitCode}`,
    )

    return {
      resultStatus: 'Complete',
      completion: 100,
      notes: forbidden
        ? notes + `\n\nTEST-MODE VIOLATION (SCOPED): the model reported the forbidden FULL-regression command \`${forbidden}\`. The runtime policy is authoritative; FULL requires explicit runtime authorization (test-mode: FULL).`
        : notes,
      testsSummary: forbidden
        ? `opencode exit code ${result.exitCode} | TEST-MODE VIOLATION (SCOPED): ${forbidden}`
        : testsSummary,
      commitHash,
      runtimeAdapter: this.runtimeAdapterId,
      modelProfile: command.modelProfile,
      externalRunId: this.externalRunId,
      executionEnvironment: command.executionEnvironment ?? null,
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
      note: `opencode run executing (external ${this.externalRunId})`,
    }
  }
}
