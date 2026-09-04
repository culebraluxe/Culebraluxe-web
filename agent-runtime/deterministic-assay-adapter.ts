import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
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
  assayPlanFromInstructions,
  finalizeAssayEvidence,
  parseAssayTestCounters,
  type AssayCommandResult,
  type AssayEvidence,
} from './assay-evidence'
import { detectFullRegressionAttempt } from './test-mode'
import {
  assertExecutionTargetSafe,
  buildChildProcessEnv,
  parseExecutionEnvironment,
  verifyWorkspaceEnvFile,
} from '../lib/execution-target'

const OUTPUT_TAIL_MAX = 8_000
const DEFAULT_COMMAND_TIMEOUT_MS = 15 * 60 * 1000

export type AssayCommandRunner = (input: {
  command: string
  cwd: string
  env: Record<string, string | undefined>
  timeoutMs: number
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

/**
 * Execute exactly one immutable Assay command. Process exit state is recorded
 * as data; a non-zero test exit is NOT a harness/runtime crash.
 */
export async function runAssayCommand(input: {
  command: string
  cwd: string
  env: Record<string, string | undefined>
  timeoutMs: number
}): Promise<AssayCommandResult> {
  const started = Date.now()
  let stdoutTail = ''
  let stderrTail = ''
  let timedOut = false

  return await new Promise<AssayCommandResult>((resolve) => {
    let proc: ChildProcessWithoutNullStreams | null = null
    let settled = false
    const finish = (result: Omit<AssayCommandResult, 'durationMs' | 'tests'>) => {
      if (settled) return
      settled = true
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
      }) as ChildProcessWithoutNullStreams
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

    proc.stdout.on('data', (chunk) => {
      stdoutTail = appendTail(stdoutTail, String(chunk))
    })
    proc.stderr.on('data', (chunk) => {
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

    const timeout = setTimeout(() => {
      if (settled || !proc) return
      timedOut = true
      proc.kill('SIGTERM')
    }, input.timeoutMs)
    timeout.unref?.()
    proc.once('close', () => clearTimeout(timeout))
    proc.once('error', () => clearTimeout(timeout))
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

/**
 * Forge V6 Assay runtime.
 *
 * No model is invoked. Forge executes the architect-approved commands in the
 * exact candidate worktree, records process/test facts, applies arithmetic,
 * and returns PASS/Hold. Human prose is presentation only.
 */
export class DeterministicAssayAdapter extends AgentRuntimeAdapter {
  readonly runtimeAdapterId = 'forge-assay'
  readonly capabilities: AgentCapability[] = ASSAY_CAPABILITIES

  private evidence: AssayEvidence | null = null
  private execution: Promise<void> | null = null
  private done = false
  private cancelled = false
  private currentChild: ChildProcessWithoutNullStreams | null = null

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
    this.execution = this.executePlan(context).finally(() => {
      this.done = true
    })
    return { externalRunId }
  }

  private async executePlan(context: AgentExecutionContext): Promise<void> {
    const startedAt = new Date().toISOString()
    const instructions = context.command.specialInstructions
    const plan = assayPlanFromInstructions(instructions)
    const candidateSha = assayCandidateFromInstructions(instructions)
    const workspace = context.executionWorkspace?.worktreePath ?? null
    const verifiedSha = workspace ? gitHead(workspace) : null
    const policyViolations: string[] = []
    const commandResults: AssayCommandResult[] = []
    const requiredCommands = plan?.commands ?? []

    if (!workspace) {
      policyViolations.push(
        'Assay requires an isolated exact-candidate worktree; no execution workspace was provided.',
      )
    }
    if (!plan || requiredCommands.length === 0) {
      policyViolations.push('Assay immutable command plan is missing or invalid.')
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

    if (workspace && policyViolations.length === 0) {
      const target = parseExecutionEnvironment(
        context.executionEnvironment ?? context.command.executionEnvironment,
        'DEV',
      )
      try {
        assertExecutionTargetSafe(target)
        verifyWorkspaceEnvFile(workspace, target)
      } catch (error) {
        policyViolations.push(
          `Execution-target safety rejected Assay: ${String((error as Error)?.message ?? error)}`,
        )
      }

      if (policyViolations.length === 0) {
        const env = buildChildProcessEnv(target)
        const timeoutMs =
          this.config.commandTimeoutMs ??
          Number(process.env.FORGE_ASSAY_COMMAND_TIMEOUT_MS ?? DEFAULT_COMMAND_TIMEOUT_MS)
        const runner = this.config.runCommand ?? runAssayCommand

        for (const command of requiredCommands) {
          if (this.cancelled) break
          const result = await runner({
            command,
            cwd: workspace,
            env,
            timeoutMs:
              Number.isFinite(timeoutMs) && timeoutMs > 0
                ? timeoutMs
                : DEFAULT_COMMAND_TIMEOUT_MS,
          })
          commandResults.push(result)
          // Fail fast. A broken command cannot be repaired by later commands.
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
    }

    if (this.cancelled) {
      policyViolations.push('Assay cancelled by operator.')
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
  }

  protected async statusExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<ExternalStatusResult> {
    if (this.cancelled) return { lifecycle: 'cancelled' }
    if (!this.execution || !this.done) return { lifecycle: 'running' }
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

  protected override progressFromStatus(): {
    step?: string
    completion?: number
    note?: string
  } | null {
    return {
      step: 'running_tests',
      completion: 50,
      note: 'Forge deterministic Assay executing immutable verification commands',
    }
  }
}
