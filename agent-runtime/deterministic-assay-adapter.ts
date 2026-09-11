import { spawn, type ChildProcess } from 'node:child_process'
import { execFileSync } from 'node:child_process'

import {
  AgentRuntimeAdapter,
  type ExternalStartResult,
  type ExternalStatusResult,
} from './agent-runtime-adapter'
import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from './types'
import { ASSAY_CAPABILITIES } from './lanes'
import type { AgentCapability } from './capabilities'
import {
  assayCandidateFromInstructions,
  assayEvidenceSummary,
  finalizeAssayEvidence,
  parseAssayTestCounters,
  type AssayCommandResult,
  type AssayEvidence,
} from './assay-evidence'
import { planAssay } from './assay-plan'
import { detectFullRegressionAttempt } from './test-mode'
import { recordStaticGateArtifact, recordAssayEvidenceArtifact } from '../db/forge-artifact'
import { runStaticGate } from '../workflow_app/forge/forge-static-gate'
import {
  assertExecutionTargetSafe,
  buildChildProcessEnv,
  parseExecutionEnvironment,
  verifyWorkspaceEnvFile,
} from '../lib/execution-target'

const OUTPUT_TAIL_MAX = 8_000
const DEFAULT_COMMAND_TIMEOUT_MS = 15 * 60 * 1000
const STATUS_POLL_MS = 250

export type AssayCommandRunner = (input: {
  command: string
  cwd: string
  env: Record<string, string | undefined>
  timeoutMs: number
  onProcess?: (proc: ChildProcess | null) => void
}) => Promise<AssayCommandResult>

export type DeterministicAssayConfig = {
  runCommand?: AssayCommandRunner
  commandTimeoutMs?: number
}

function appendTail(current: string, chunk: string): string {
  const next = current + chunk
  return next.length <= OUTPUT_TAIL_MAX
    ? next
    : next.slice(next.length - OUTPUT_TAIL_MAX)
}

/** Execute one immutable command and return process/test facts, not a verdict. */
export async function runAssayCommand(input: {
  command: string
  cwd: string
  env: Record<string, string | undefined>
  timeoutMs: number
  onProcess?: (proc: ChildProcess | null) => void
}): Promise<AssayCommandResult> {
  const started = Date.now()
  let stdoutTail = ''
  let stderrTail = ''
  let timedOut = false

  return await new Promise<AssayCommandResult>((resolve) => {
    let proc: ChildProcess | null = null
    let settled = false
    const finish = (result: Omit<AssayCommandResult, 'durationMs' | 'tests'>) => {
      if (settled) return
      settled = true
      input.onProcess?.(null)
      const combined = `${stdoutTail}\n${stderrTail}`
      resolve({
        ...result,
        durationMs: Date.now() - started,
        tests: parseAssayTestCounters(combined),
      })
    }

    try {
      proc = spawn(input.command, {
        cwd: input.cwd,
        env: input.env as NodeJS.ProcessEnv,
        shell: process.env.FORGE_ASSAY_SHELL ?? process.env.SHELL ?? true,
        stdio: ['ignore', 'pipe', 'pipe'],
        // Own process group so a timeout can kill the WHOLE tree, not just the
        // shell. A shell child (the actual test process) that survives would keep
        // the pipes open and prevent 'close' - hanging QA past the timeout.
        detached: process.platform !== 'win32',
      })
      input.onProcess?.(proc)
    } catch (error) {
      stderrTail = appendTail(
        stderrTail,
        String((error as Error)?.message ?? error),
      )
      finish({
        command: input.command,
        exitCode: null,
        signal: null,
        timedOut: false,
        stdoutTail,
        stderrTail,
      })
      return
    }

    proc.stdout?.on('data', (chunk) => {
      stdoutTail = appendTail(stdoutTail, String(chunk))
    })
    proc.stderr?.on('data', (chunk) => {
      stderrTail = appendTail(stderrTail, String(chunk))
    })
    proc.on('error', (error) => {
      stderrTail = appendTail(stderrTail, String(error.message ?? error))
      finish({
        command: input.command,
        exitCode: null,
        signal: null,
        timedOut,
        stdoutTail,
        stderrTail,
      })
    })
    proc.on('close', (code, signal) => {
      finish({
        command: input.command,
        exitCode: code,
        signal: signal ?? null,
        timedOut,
        stdoutTail,
        stderrTail,
      })
    })

    // Hard termination guarantee: a command must NEVER leave QA hanging. On
    // timeout, SIGTERM the whole process group, then SIGKILL and force a timed-
    // out terminal result shortly after - so even a child that ignores SIGTERM
    // or keeps a pipe open cannot stall QA past the grace window.
    let bail: NodeJS.Timeout | null = null
    const killTree = (signal: NodeJS.Signals): void => {
      if (!proc || proc.pid == null) return
      try {
        process.kill(-proc.pid, signal)
      } catch {
        /* group already gone */
      }
      try {
        proc.kill(signal)
      } catch {
        /* ignore */
      }
    }
    const timeout = setTimeout(() => {
      if (settled || !proc) return
      timedOut = true
      killTree('SIGTERM')
      bail = setTimeout(() => {
        killTree('SIGKILL')
        finish({
          command: input.command,
          exitCode: null,
          signal: 'SIGTERM',
          timedOut: true,
          stdoutTail,
          stderrTail,
        })
      }, 5000)
    }, input.timeoutMs)
    const clearTimers = (): void => {
      clearTimeout(timeout)
      if (bail) clearTimeout(bail)
    }
    proc.once('close', clearTimers)
    proc.once('error', clearTimers)
  })
}

function gitHead(cwd: string): string | null {
  try {
    const value = execFileSync('git', ['rev-parse', 'HEAD'], {
      cwd,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim().toLowerCase()
    return /^[0-9a-f]{40}$/.test(value) ? value : null
  } catch {
    return null
  }
}

function failedCommandResult(command: string, error: unknown): AssayCommandResult {
  return {
    command,
    exitCode: null,
    signal: null,
    timedOut: false,
    durationMs: 0,
    tests: { total: null, passed: null, failed: null },
    stdoutTail: '',
    stderrTail: String((error as Error)?.message ?? error),
  }
}

/** Forge V6 model-free exact-candidate verifier. */
export class DeterministicAssayAdapter extends AgentRuntimeAdapter {
  readonly runtimeAdapterId = 'forge-assay'
  readonly capabilities: AgentCapability[] = ASSAY_CAPABILITIES

  private evidence: AssayEvidence | null = null
  private execution: Promise<void> | null = null
  private done = false
  private cancelled = false
  private currentChild: ChildProcess | null = null

  constructor(
    deps: ConstructorParameters<typeof AgentRuntimeAdapter>[0],
    private readonly config: DeterministicAssayConfig = {},
  ) {
    super(deps)
  }

  protected async startExternal(
    context: AgentExecutionContext,
  ): Promise<ExternalStartResult> {
    const externalRunId = `forge-assay-${context.storyRunId}`
    this.externalRunId = externalRunId
    this.done = false
    this.cancelled = false
    this.evidence = null
    this.execution = this.executePlan(context)
      .catch(async (error) => {
        const workspace = context.executionWorkspace?.worktreePath ?? null
        const verifiedSha = workspace ? gitHead(workspace) : null
        const candidateSha =
          assayCandidateFromInstructions(context.command.specialInstructions) ??
          verifiedSha
        const frozenPlan = planAssay({
          testMode: context.story.testMode,
          assayCommands: context.story.assayCommands,
        })
        this.evidence = finalizeAssayEvidence({
          version: 1,
          candidateSha,
          verifiedSha,
          requiredCommands: frozenPlan.ok ? frozenPlan.commands : [],
          commandResults: [],
          policyViolations: [
            `Assay runtime exception: ${String((error as Error)?.message ?? error)}`,
          ],
          startedAt: new Date().toISOString(),
          endedAt: new Date().toISOString(),
        })
        await recordAssayEvidenceArtifact({
          storyId: context.story.id,
          storyRunId: context.storyRunId,
          evidence: this.evidence,
        }).catch(() => {
          /* artifact write is best-effort; the QA verdict stands on the tree */
        })
      })
      .finally(() => {
        this.currentChild = null
        this.done = true
      })
    return { externalRunId }
  }

  private async executePlan(context: AgentExecutionContext): Promise<void> {
    const startedAt = new Date().toISOString()
    const instructions = context.command.specialInstructions
    const frozenPlan = planAssay({
      testMode: context.story.testMode,
      assayCommands: context.story.assayCommands,
    })
    const plan = frozenPlan.ok ? frozenPlan : null
    const workspace = context.executionWorkspace?.worktreePath ?? null
    const verifiedSha = workspace ? gitHead(workspace) : null
    // Fall back to the worktree HEAD when no explicit candidate directive was
    // stamped: the QA worktree IS the Smith candidate. Without this, an engine
    // that does not pin the directive can never PASS (CANDIDATE_MISMATCH -> the
    // infinite repair-Smith loop). Exact-SHA pinning still wins when present.
    const candidateSha =
      assayCandidateFromInstructions(instructions) ?? verifiedSha
    const policyViolations: string[] = []
    const commandResults: AssayCommandResult[] = []
    const requiredCommands = plan?.commands ?? []

    if (!workspace) {
      policyViolations.push(
        'Assay requires an isolated exact-candidate worktree; no execution workspace was provided.',
      )
    }
    if (!plan || requiredCommands.length === 0) {
      policyViolations.push(
        frozenPlan.ok
          ? 'Assay frozen Story Run command plan is empty.'
          : `Assay frozen Story Run plan is invalid: ${frozenPlan.reason}`,
      )
    }
    if (!candidateSha) {
      policyViolations.push('Assay immutable candidate SHA is missing or invalid.')
    }

    if (plan?.mode !== 'FULL') {
      for (const command of requiredCommands) {
        const forbidden = detectFullRegressionAttempt(command)
        if (forbidden) {
          policyViolations.push(
            `Assay command violates ${plan?.mode ?? 'SCOPED'} test policy: ${forbidden}`,
          )
        }
      }
    }

    let env: Record<string, string | undefined> | null = null
    if (workspace && policyViolations.length === 0) {
      try {
        const target = parseExecutionEnvironment(
          context.executionEnvironment ?? context.command.executionEnvironment,
          'DEV',
        )
        assertExecutionTargetSafe(target)
        verifyWorkspaceEnvFile(workspace, target)
        env = buildChildProcessEnv(target)
      } catch (error) {
        policyViolations.push(
          `Execution-target safety rejected Assay: ${String((error as Error)?.message ?? error)}`,
        )
      }
    }

    if (workspace && env && policyViolations.length === 0) {
      const timeoutMs =
        this.config.commandTimeoutMs ??
        Number(process.env.FORGE_ASSAY_COMMAND_TIMEOUT_MS ?? DEFAULT_COMMAND_TIMEOUT_MS)
      const runner = this.config.runCommand ?? runAssayCommand

      for (const command of requiredCommands) {
        if (this.cancelled) break
        let result: AssayCommandResult
        try {
          result = await runner({
            command,
            cwd: workspace,
            env,
            timeoutMs:
              Number.isFinite(timeoutMs) && timeoutMs > 0
                ? timeoutMs
                : DEFAULT_COMMAND_TIMEOUT_MS,
            onProcess: (proc) => {
              this.currentChild = proc
            },
          })
        } catch (error) {
          result = failedCommandResult(command, error)
        }
        commandResults.push(result)

        if (
          result.timedOut ||
          result.exitCode !== 0 ||
          (result.tests.failed !== null && result.tests.failed !== 0) ||
          (result.tests.total !== null &&
            result.tests.passed !== null &&
            result.tests.total !== result.tests.passed)
        ) {
          break
        }
      }
    }

    if (this.cancelled) policyViolations.push('Assay cancelled by operator.')

    // Static gate (architecture + security) on the exact candidate: runs AFTER the
    // targeted commands pass so we gate on a coherent tree. Architecture is the
    // hard gate; a clean result (or any arch violation) is recorded durably as a
    // forge_tool_artifact child under the story run. Semgrep reports informationally.
    const allCommandsPassed =
      policyViolations.length === 0 &&
      commandResults.length === requiredCommands.length &&
      commandResults.every((r) => r.exitCode === 0)
    if (workspace && allCommandsPassed && !this.cancelled) {
      try {
        const gate = runStaticGate({ workspace, config: '.dependency-cruiser.js' })
        if (!gate.archOk) {
          for (const error of gate.archErrors.slice(0, 8)) {
            policyViolations.push(`Static gate (dependency-cruiser): ${error}`)
          }
        }
        // Durable evidence BEFORE QA evidence finalizes (observer; a failed write
        // must never break QA — DB failures are already captured at the gateway).
        await recordStaticGateArtifact({
          storyId: context.story.id,
          storyRunId: context.storyRunId,
          sha: candidateSha ?? verifiedSha ?? null,
          archOk: gate.archOk,
          archErrorCount: gate.archErrors.length,
          semgrepRan: gate.semgrepRan,
          semgrepFindingCount: gate.semgrepFindings.length,
          knipRan: gate.knipRan,
          knipFindingCount: gate.knipFindings.length,
          workspace,
        }).catch(() => {
          /* artifact write is best-effort; QA verdict stands on the tree */
        })
      } catch (error) {
        policyViolations.push(
          `Static gate runtime error: ${String((error as Error)?.message ?? error)}`,
        )
      }
    }

    this.evidence = finalizeAssayEvidence({
      version: 1,
      candidateSha,
      verifiedSha,
      requiredCommands,
      commandResults,
      policyViolations,
      startedAt,
      endedAt: new Date().toISOString(),
    })
    // Durable reason for the verdict (best-effort). Without this a QA FAIL leaves
    // only CODE_DEFECT behind and cannot be diagnosed after the fact.
    await recordAssayEvidenceArtifact({
      storyId: context.story.id,
      storyRunId: context.storyRunId,
      evidence: this.evidence,
    }).catch(() => {
      /* artifact write is best-effort; the QA verdict stands on the tree */
    })
  }

  protected async statusExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<ExternalStatusResult> {
    if (this.cancelled) return { lifecycle: 'cancelled' }
    if (!this.execution || !this.done) {
      await new Promise((resolve) => setTimeout(resolve, STATUS_POLL_MS))
      return { lifecycle: this.cancelled ? 'cancelled' : 'running' }
    }
    await this.execution
    return { lifecycle: 'success' }
  }

  protected async pauseExternal(): Promise<void> {
    this.currentChild?.kill('SIGSTOP')
  }

  protected async resumeExternal(): Promise<void> {
    this.currentChild?.kill('SIGCONT')
  }

  protected async cancelExternal(): Promise<void> {
    this.cancelled = true
    this.currentChild?.kill('SIGTERM')
  }

  protected async resultExternal(
    command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null> {
    if (this.execution) await this.execution
    const evidence = this.evidence
    if (!evidence) return null
    const pass = evidence.verdict === 'PASS'
    const summary = assayEvidenceSummary(evidence)
    return {
      resultStatus: pass ? 'Complete' : 'Hold',
      completion: 100,
      notes: summary,
      testsSummary: summary,
      commitHash: null,
      assayEvidence: evidence,
      runtimeAdapter: this.runtimeAdapterId,
      modelProfile: command.modelProfile,
      externalRunId: this.externalRunId,
      executionEnvironment: command.executionEnvironment ?? null,
      startedAt: evidence.startedAt,
      endedAt: evidence.endedAt,
    }
  }
}
