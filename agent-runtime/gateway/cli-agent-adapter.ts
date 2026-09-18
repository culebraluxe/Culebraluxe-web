import { execFileSync, spawn, type ChildProcess } from 'node:child_process'
import { gitBinary } from '../../lib/worker-workspace/provisioner'

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
import {
  commitWorkerWorkspaceChanges,
  parseAllowedScopeMarker,
} from '../../lib/worker-workspace/commit'

const FORGE_OWNED_ENV_KEYS = new Set([
  'APP_ENV',
  'EXECUTION_ENV',
  'DATABASE_URL',
  'DATABASE_URL_DEV',
  'DATABASE_URL_PROD',
])

export function buildGatewayChildEnv(
  target: ExecutionEnvironment,
  providerEnv: Record<string, string | undefined> = {},
  baseEnv: NodeJS.ProcessEnv = process.env,
): Record<string, string | undefined> {
  const allowedProviderEnv = Object.fromEntries(
    Object.entries(providerEnv).filter(([key]) => !FORGE_OWNED_ENV_KEYS.has(key)),
  )
  return buildChildProcessEnv(target, { ...baseEnv, ...allowedProviderEnv })
}

/**
 * ENG-FORGE-REVIEW-RESIDUALS-01 — the CLI gateway's commit options come from the
 * SAME declared-surface marker the factory path reads (`parseAllowedScopeMarker`),
 * so the two harness-owned commit paths cannot drift into two scope rules. An
 * absent/unparseable marker preserves the legacy commit-everything behaviour.
 */
export function adapterCommitOptions(
  specialInstructions: string | null | undefined,
): { allowedScope?: string[] } {
  const allowedScope = parseAllowedScopeMarker(specialInstructions)
  return allowedScope ? { allowedScope } : {}
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
    // Keep the spawned process in a local non-null binding while wiring its
    // listeners. Next augments NodeJS.ProcessEnv with required framework keys,
    // while our sanitized runtime env is intentionally a generic string map;
    // the cast is only at Node's spawn boundary and does not change runtime
    // values or the execution-target safety policy.
    const proc = spawn(command.bin, command.args, {
      cwd,
      env: childEnv as NodeJS.ProcessEnv,
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    this.proc = proc
    proc.stdout?.on('data', (chunk) => { this.stdout += String(chunk) })
    proc.stderr?.on('data', (chunk) => { this.stderr += String(chunk) })
    proc.on('error', (error) => { this.externalErrorText = error.message })
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
    let refusedPaths: string[] = []

    if (context.policy.allowCommit && context.executionWorkspace) {
      try {
        const head = execFileSync(gitBinary(), ['rev-parse', 'HEAD'], { cwd, encoding: 'utf8' }).trim()
        const base = context.executionWorkspace.baseCommit
        commitHash = head && head !== base ? head : null
        if (!commitHash) {
          const committed = await commitWorkerWorkspaceChanges(
            cwd,
            `${context.story.id}: ${context.story.title}`,
            adapterCommitOptions(context.command.specialInstructions),
          )
          commitHash = committed.commitHash
          refusedPaths = committed.refused ?? []
        }
      } catch {
        commitHash = null
      }
    } else if (context.policy.allowCommit) {
      try {
        commitHash = execFileSync(gitBinary(), ['rev-parse', 'HEAD'], { cwd, encoding: 'utf8' }).trim() || null
      } catch {
        commitHash = null
      }
    }

    const output = this.stdout.trim()
    const refusedNote =
      refusedPaths.length > 0
        ? `Commit refused: path(s) outside the declared scope — ${refusedPaths.join(', ')}`
        : null
    return {
      resultStatus: 'Complete',
      completion: 100,
      notes: [output || `${this.provider.id} completed successfully`, refusedNote]
        .filter(Boolean)
        .join('\n'),
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
