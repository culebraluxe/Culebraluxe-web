import { execFileSync, spawn, type ChildProcess } from 'node:child_process'

import {
  AgentRuntimeAdapter,
  type ExternalStartResult,
  type ExternalStatusResult,
} from '../agent-runtime-adapter'
import type { AgentCapability } from '../capabilities'
import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from '../types'
import type { ForgeProviderDescriptor } from './provider'
import { buildTaskText, extractTestsSummary } from '../deepseek/deepseek-harness-adapter'
import {
  buildChildProcessEnv,
  parseExecutionEnvironment,
  verifyWorkspaceEnvFile,
  type ExecutionEnvironment,
} from '../../lib/execution-target'

/**
 * Build the environment for a gateway child process. Provider-specific values
 * (API keys, CLI config, etc.) are accepted as input, then the canonical Forge
 * execution target is applied LAST by buildChildProcessEnv so a provider can
 * never override APP_ENV / EXECUTION_ENV / database targeting.
 */
export function buildGatewayChildEnv(
  target: ExecutionEnvironment,
  providerEnv: Record<string, string | undefined> = {},
  baseEnv: NodeJS.ProcessEnv = process.env,
): Record<string, string | undefined> {
  return buildChildProcessEnv(target, { ...baseEnv, ...providerEnv })
}

export class CliAgentGatewayAdapter extends AgentRuntimeAdapter {
  readonly runtimeAdapterId: string
  readonly capabilities: AgentCapability[]

  private proc: ChildProcess | null = null
  private stdout = ''
  private stderr = ''

  constructor(
    deps: ConstructorParameters<typeof AgentRuntimeAdapter>[0],
    private readonly provider: ForgeProviderDescriptor,
  ) {
    super(deps)
    this.runtimeAdapterId = `gateway-${provider.id}`
    this.capabilities = provider.capabilities
  }

  protected async startExternal(context: AgentExecutionContext): Promise<ExternalStartResult> {
    const cwd = context.executionWorkspace?.worktreePath ?? process.cwd()
    const target = parseExecutionEnvironment(context.executionEnvironment, 'DEV')

    // Safety parity with the native DeepSeek harness: inspect the exact worker
    // checkout before spawning any external executor, then build a child env
    // that cannot inherit/reintroduce a PROD database target for DEV/LOCAL/TEST.
    verifyWorkspaceEnvFile(cwd, target)

    const task = buildTaskText(context.command, context)
    const command = this.provider.buildCommand({
      cwd,
      task,
      modelProfile: context.command.modelProfile,
    })
    const childEnv = buildGatewayChildEnv(target, command.env ?? {})

    this.stdout = ''
    this.stderr = ''
    this.proc = spawn(command.bin, command.args, {
      cwd,
      env: childEnv,
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    this.proc.stdout?.on('data', (chunk) => { this.stdout += String(chunk) })
    this.proc.stderr?.on('data', (chunk) => { this.stderr += String(chunk) })
    this.proc.on('error', (error) => { this.externalErrorText = error.message })
    const externalRunId = `${this.provider.id}-${context.command.workItemId}-${Date.now()}`
    return { externalRunId }
  }

  protected async statusExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<ExternalStatusResult> {
    if (!this.proc) return { lifecycle: 'failed' }
    if (this.proc.exitCode === null && !this.proc.killed) return { lifecycle: 'running' }
    if (this.proc.killed) return { lifecycle: 'cancelled' }
    if (this.proc.exitCode === 0) return { lifecycle: 'success' }
    this.externalErrorText = this.stderr.trim() || `${this.provider.id} exited ${this.proc.exitCode}`
    return { lifecycle: 'failed' }
  }

  protected async pauseExternal(): Promise<void> {
    throw new Error(`${this.provider.id} gateway adapter does not support pause`)
  }

  protected async resumeExternal(): Promise<void> {
    throw new Error(`${this.provider.id} gateway adapter does not support resume`)
  }

  protected async cancelExternal(): Promise<void> {
    this.proc?.kill('SIGTERM')
  }

  protected async resultExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null> {
    const cwd = context.executionWorkspace?.worktreePath ?? process.cwd()
    let commitHash: string | null = null
    if (context.policy.allowCommit) {
      try {
        const head = execFileSync('git', ['rev-parse', 'HEAD'], { cwd, encoding: 'utf8' }).trim()
        const base = context.executionWorkspace?.baseCommit ?? null
        commitHash = head && head !== base ? head : null
      } catch {
        commitHash = null
      }
    }
    const output = this.stdout.trim()
    return {
      resultStatus: 'Complete',
      completion: 100,
      notes: output || `${this.provider.id} completed successfully`,
      testsSummary: extractTestsSummary(output, `${this.provider.id} exit 0`),
      commitHash,
      runtimeAdapter: this.runtimeAdapterId,
      modelProfile: command.modelProfile,
      externalRunId: this.externalRunId,
      executionEnvironment: context.executionEnvironment ?? null,
      startedAt: new Date().toISOString(),
      endedAt: new Date().toISOString(),
    }
  }
}
