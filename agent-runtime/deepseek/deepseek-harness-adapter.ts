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
  discoverLatestSession,
  type DshHandle,
  type DshRunResult,
} from './dsh-client'
import {
  detectFullRegressionAttempt,
  resolveTestModeFromInstructions,
  testModeTaskPolicy,
} from '../test-mode'
import { buildChildProcessEnv } from '../../lib/execution-target'
import { execFileSync } from 'node:child_process'

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
  // The runtime test execution mode is authoritative and OUTRANKS story prose.
  // The directive is consumed here (stripped) so the model never sees the token.
  const { mode, instructions } = resolveTestModeFromInstructions(
    command.specialInstructions,
    process.env.AGENT_TEST_MODE ?? null,
  )
  const parts: string[] = []
  parts.push(`Execute SDLC story ${s.id}: ${s.title}.`)
  if (s.goal) parts.push(`Goal: ${s.goal}`)
  if (instructions) {
    parts.push(`Special instructions (additive, do not replace the architect brief): ${instructions}`)
  }
  if (s.architectBrief) parts.push(`Architect brief: ${s.architectBrief}`)
  if (s.acceptanceCriteria) parts.push(`Acceptance criteria (do not mark Complete unless these are satisfied): ${s.acceptanceCriteria}`)
  parts.push(testModeTaskPolicy(mode))
  parts.push(
    'Work in the current repository. Verify your work by running tests/typecheck/build within the runtime policy above. ' +
    'Create a local git commit with the intended changes when the story requires it. ' +
    'Do NOT push. Do NOT mutate production data or schema. Report what you did.',
  )
  // ENG-08 — story execution evidence: ask the model to END its final report
  // with one concise machine-scannable tests line so the harness can persist a
  // concrete tests/checks summary (not just an exit code) against the story
  // run. The example deliberately names targeted single-file suites only —
  // never the forbidden full-regression globs.
  parts.push(
    'End your final report with one concise "Tests: <summary>" line (e.g. "Tests: workflow_app/tests/evidence-summary.test.ts 8/8 pass; tsc --noEmit clean") so the harness can record a concrete tests/checks summary against this story.',
  )
  return parts.join('\n')
}

// ---------------------------------------------------------------------------
// ENG-08 — concise tests/checks summary extraction (story execution evidence).
// ---------------------------------------------------------------------------

/** Marker the task text asks the model to end its report with. */
export const TESTS_SUMMARY_MARKER = 'Tests:'

/** Upper bound for the persisted tests summary — concise by contract, never a
 *  transcript dump. Longer summaries are truncated with an ellipsis. */
export const TESTS_SUMMARY_MAX_LENGTH = 300

/**
 * Extract the concise tests/checks summary from the assistant's final report.
 *
 * The task text asks the model to END the report with a single `Tests:
 * <summary>` line. This scans each line for the marker and keeps the LAST
 * non-empty match (a late, deliberate evidence line wins over earlier prose
 * mentions), returning its trimmed content. Absent or malformed markers fall
 * back to the caller-provided factual fallback (the harness exit code) — the
 * tests summary is never fabricated from free-form prose.
 */
export function extractTestsSummary(
  output: string | null | undefined,
  fallback: string,
): string {
  if (!output) return fallback
  let summary: string | null = null
  for (const rawLine of output.split(/\r?\n/)) {
    const match = rawLine.match(new RegExp(`\\b${TESTS_SUMMARY_MARKER}\\s+(.+)$`))
    if (!match) continue
    const value = match[1].trim()
    if (value) summary = value
  }
  if (summary === null) return fallback
  if (summary.length <= TESTS_SUMMARY_MAX_LENGTH) return summary
  return `${summary.slice(0, TESTS_SUMMARY_MAX_LENGTH - 1)}…`
}

export class DeepSeekHarnessAdapter extends AgentRuntimeAdapter {
  readonly runtimeAdapterId = 'deepseek-harness'
  readonly capabilities: AgentCapability[] = DEEPSEEK_CAPABILITIES

  private handle: DshHandle | null = null
  private lastResult: DshRunResult | null = null
  /** Newest DSH session present in the workspace BEFORE this run spawned —
   * used to reject a stale pre-run session during in-run discovery. */
  private sessionBaseline: string | null = null

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
    // Defensive FAIL-FAST (ENG-20/ENG-20A): the adapter must never spawn
    // external work for an execution target whose application/domain DB
    // configuration would resolve to the production database. The invoker
    // already guards before calling execute; this is the second, adapter-level
    // barrier directly before the harness process is started.
    const target = (context.executionEnvironment ?? 'DEV') as never
    const { assertExecutionTargetSafe, buildChildProcessEnv, verifyWorkspaceEnvFile } = await import('../../lib/execution-target')
    assertExecutionTargetSafe(target)
    verifyWorkspaceEnvFile(this.config.workspace, target)

    const task = this.taskBuilder(context.command, context)
    const startRun = this.config.startRun ?? startDshRun
    // DEV safety: the spawned harness (and any test process it spawns) must
    // NOT inherit an APP_ENV/DATABASE_URL set that resolves to the production
    // application database. buildChildProcessEnv builds a sanitized child env
    // (APP_ENV/EXECUTION_ENV/DATABASE_URL forced to the DEV target; the PROD
    // url removed) and FAILS FAST on a DEV->PROD mismatch before spawn.
    const childEnv = buildChildProcessEnv(target)
    this.handle = startRun({
      cliBin: this.config.cliBin,
      cwd: this.config.workspace,
      task,
      env: { ...childEnv, ...(this.config.env ?? {}) },
    })

    // Opaque correlation: the harness generates session-<uuid> internally and
    // does NOT print it. The real session is discovered during the run by
    // statusExternal and persisted immediately (no longer only at finalization).
    // Baseline = the newest session already present BEFORE spawn, so in-run
    // discovery never mistakes a stale pre-run session for this run's session.
    this.sessionBaseline = discoverLatestSession(this.config.workspace)
    this.externalRunId = `deepseek-pending-${Date.now()}`
    return { externalRunId: this.externalRunId }
  }

  protected async statusExternal(
    command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<ExternalStatusResult> {
    if (!this.handle) return { lifecycle: 'failed' }
    // Cancellation is requested first: SIGTERM sent. The child may not have
    // exited yet; map directly to the canonical cancelled lifecycle so the
    // shared loop terminalizes without racing the process exit.
    if (this.handle.cancelled) return { lifecycle: 'cancelled' }
    if (this.handle.proc.exitCode === null) {
      // In-run DSH session discovery: once the real session-<uuid> directory
      // exists for this workspace, persist it immediately so the operator sees
      // the REAL session id while the story is still Running (the pending id is
      // only a short-lived bootstrap value).
      if (this.externalRunId?.startsWith('deepseek-pending-')) {
        const session = discoverLatestSession(this.config.workspace)
        if (session && session !== this.sessionBaseline) {
          this.externalRunId = session
          try {
            await this.deps.work.setRuntime(command.workItemId, {
              runtimeAdapter: this.runtimeAdapterId,
              externalRunId: session,
            })
          } catch {
            // Correlation is evidence only; a terminal race must not break the loop.
          }
        }
      }
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

    // The harness works inside the repo, so the parent may read the actual
    // HEAD commit the model created (factual, never inferred from the model's
    // self-report).
    let commitHash: string | null = null
    try {
      commitHash = execFileSync('git', ['log', '-1', '--format=%H'], {
        cwd: this.config.workspace,
        encoding: 'utf8',
      }).trim() || null
    } catch {
      commitHash = null
    }

    const notes = [
      'DeepSeek Harness run completed.',
      result.stdout.trim() ? `Assistant output:\n${result.stdout.trim()}` : 'No assistant text captured.',
      result.stderr.trim() ? `stderr:\n${result.stderr.trim()}` : null,
      result.sessionDir ? `Session transcript: ${result.sessionDir}` : null,
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
    const forbidden = mode === 'SCOPED'
      ? detectFullRegressionAttempt(result.stdout)
      : null

    // ENG-08 — concise tests/checks summary: prefer the model's deliberate
    // `Tests: ...` evidence line (see buildTaskText); fall back to the factual
    // harness exit code. A SCOPED violation is the headline fact and still
    // replaces the summary so the operator sees the policy breach first.
    const testsSummary = extractTestsSummary(
      result.stdout,
      `dsh exit code ${result.exitCode}`,
    )

    return {
      resultStatus: 'Complete',
      completion: 100,
      notes: forbidden
        ? notes + `\n\nTEST-MODE VIOLATION (SCOPED): the model reported the forbidden FULL-regression command \`${forbidden}\`. The runtime policy is authoritative; FULL requires explicit runtime authorization (test-mode: FULL).`
        : notes,
      testsSummary: forbidden
        ? `dsh exit code ${result.exitCode} | TEST-MODE VIOLATION (SCOPED): ${forbidden}`
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
      note: `deepseek harness running (external ${this.externalRunId})`,
    }
  }
}


