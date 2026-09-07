// ---------------------------------------------------------------------------
// Poller / Invoker (ENG-18) — the boring parent wrapper.
// ---------------------------------------------------------------------------

import { AgentRuntimeRegistry } from './registry'
import {
  rejectAgentWorkConfiguration,
  validateAgentWorkLaunchConfig,
  type AgentWorkClaim,
} from '../db/agent-work'
import type {
  AgentExecutionContext,
  AgentExecutionWorkspace,
  AgentRunEvidence,
  AgentWorkCommand,
} from './types'
import type { AgentRunRepository, AgentWorkRepository } from './repositories'
import type { AgentCapability } from './capabilities'
import {
  executionContractFailureText,
  validateExecutionContract,
} from './execution-contract'
import { storyFieldsFromBoardAndGit } from './orchestrate'
import { smithFieldFacts } from './team'
import {
  isAssayTerminalRole,
  smithCandidateSha,
} from './candidate-assay-handoff'
import { leadRunPhaseFromInstructions } from './lead-decision'
import { resolveApprovedBaseRef } from '../lib/worker-workspace'
import { provisionOrRecoverWorkerWorkspace } from '../lib/worker-workspace/recovering-provisioner'
import type {
  WorkerWorkspace,
  WorkerWorkspaceSpec,
} from '../lib/worker-workspace/types'

export type InvokerResult = {
  workItemId: string
  storyId: string
  role: string
  modelProfile: string
  runtimeAdapter: string
  evidence: AgentRunEvidence
}

export interface AgentInvokerWorkspaces {
  workerId: string
  /**
   * WORKSPACE-01 canonical serial workspace lineage = processInstanceId +
   * execution generation (or an explicit split child of it). Never a worker id.
   */
  executionId: string
  baseRef: string
  worktreesRoot?: string
  provision: (spec: WorkerWorkspaceSpec) => Promise<WorkerWorkspace>
}

export interface AgentInvokerDeps {
  work: AgentWorkRepository
  runs: AgentRunRepository
  registry: AgentRuntimeRegistry
  requiredCapabilities?: AgentCapability[]
  workspaces?: AgentInvokerWorkspaces
  enforceExecutionContract?: boolean
}

/**
 * Normal Forge execution owns one writable workspace for the whole serial
 * lifecycle. A future explicit SPLIT may pass a child id to allocate a
 * separate Smith workspace; ordinary role/work-item ids must never fan out
 * worktrees by themselves.
 */
export function resolveWorkspaceRunId(
  executionId: string,
  splitChildId?: string | null,
): string {
  const execution = executionId.trim()
  if (!execution) throw new Error('Forge workspace executionId is required')
  const split = (splitChildId ?? '').trim()
  return split ? `${execution}-split-${split}` : execution
}

function sanitizeExecutionSegment(input: string): string {
  return input
    .trim()
    .replace(/[^A-Za-z0-9._-]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

/**
 * ENG-FORGE-WORKSPACE-01 — canonical Git lineage.
 *
 * workerId is the executor/claim identity ONLY; it must not define Git lineage.
 * The serial workspace key derives from the Forge processInstanceId plus the
 * execution generation (<processInstanceId>-e<replanAttempts>). REPLAN advances
 * the generation (replanAttempts increments -> -e1), which yields a NEW clean
 * workspace. Repair stays in the same generation/workspace. Only an explicit
 * SPLIT child appends a bounded child id to fan out a writable workspace.
 */
export function forgeExecutionGenerationKey(processInstanceId: string, replanAttempts: number): string {
  const pid = sanitizeExecutionSegment(processInstanceId)
  if (!pid) throw new Error('Forge processInstanceId is required for workspace lineage')
  const gen = Math.max(0, Number.isFinite(replanAttempts) ? Math.trunc(replanAttempts) : 0)
  return `${pid}-e${gen}`
}

export function resolveForgeExecutionRunId(
  processInstanceId: string,
  replanAttempts: number,
  splitChildId?: string | null,
): string {
  return resolveWorkspaceRunId(forgeExecutionGenerationKey(processInstanceId, replanAttempts), splitChildId)
}

export function buildAgentInvokerWorkspaces(
  workerId: string,
  env: NodeJS.ProcessEnv = process.env,
  executionIdOverride?: string,
): AgentInvokerWorkspaces | undefined {
  if ((env.AGENT_WORKSPACE_DISABLED ?? '0').trim() === '1') return undefined
  const baseRef = resolveApprovedBaseRef(env)
  const worktreesRoot = (env.AGENT_WORKSPACE_WORKTREES_ROOT ?? '').trim() || undefined
  const executionId = executionIdOverride?.trim()
    ? executionIdOverride.trim()
    : resolveWorkspaceRunId(workerId)
  return {
    workerId,
    executionId,
    baseRef,
    ...(worktreesRoot ? { worktreesRoot } : {}),
    provision: provisionOrRecoverWorkerWorkspace,
  }
}

export async function claimNextAgentCommand(
  workerId: string,
  deps: AgentInvokerDeps,
): Promise<AgentWorkClaim | null> {
  return deps.work.claimNext(workerId)
}

export async function executeClaimedAgentCommand(
  workerId: string,
  claim: AgentWorkClaim,
  deps: AgentInvokerDeps,
): Promise<InvokerResult> {
  const workItem = claim.workItem
  const story = claim.story
  const leadPhase =
    workItem.role === 'lead'
      ? leadRunPhaseFromInstructions(workItem.specialInstructions)
      : null

  const launchError = validateAgentWorkLaunchConfig(workItem)
  if (launchError) {
    await rejectAgentWorkConfiguration(workItem.id, launchError)
    throw new Error(`launch guard: ${launchError}`)
  }

  if (deps.enforceExecutionContract && workItem.role === 'builder') {
    const merged = storyFieldsFromBoardAndGit(story, story.id)
    const contract = validateExecutionContract({
      story: merged,
      executionTarget: workItem.executionEnvironment,
      modelProfile: workItem.modelProfile,
      registry: deps.registry,
      field: smithFieldFacts(),
    })
    if (!contract.ok) {
      const evidence = `execution contract gate: ${executionContractFailureText(contract) ?? contract.code}`
      await rejectAgentWorkConfiguration(workItem.id, evidence)
      throw new Error(evidence)
    }
  }

  const modelProfile = workItem.modelProfile as string
  const profileConfig = deps.registry.resolveProfile(modelProfile)

  if (deps.requiredCapabilities?.length) {
    const missing = deps.requiredCapabilities.filter(
      (c) => !profileConfig.capabilities.includes(c),
    )
    if (missing.length > 0) {
      throw new Error(
        `profile '${modelProfile}' adapter '${profileConfig.adapterId}' lacks required capability ${missing.join(',')}`,
      )
    }
  }

  const adapter = deps.registry.resolveAdapter(modelProfile, {
    work: deps.work,
    runs: deps.runs,
  })

  await deps.work.setRuntime(workItem.id, {
    runtimeAdapter: adapter.runtimeAdapterId,
  })

  const command: AgentWorkCommand = {
    workItemId: workItem.id,
    storyId: workItem.storyId,
    role: (workItem.role ?? 'builder') as AgentWorkCommand['role'],
    modelProfile,
    specialInstructions: workItem.specialInstructions ?? null,
    priority: workItem.priority,
    state: workItem.state,
    claimedBy: workItem.claimedBy,
    claimedAt: workItem.claimedAt,
    startedAt: workItem.startedAt,
    finishedAt: workItem.finishedAt,
    storyRunId: workItem.storyRunId,
    errorText: workItem.errorText,
    runtimeAdapter: workItem.runtimeAdapter,
    externalRunId: workItem.externalRunId,
    attempts: workItem.attempts,
    maxAttempts: workItem.maxAttempts,
    executionEnvironment: workItem.executionEnvironment ?? null,
    createdAt: workItem.createdAt,
    updatedAt: workItem.updatedAt,
  }

  // Lead PRE is a judgment/veto gate and must not mutate implementation.
  // Lead IMPLEMENT and POST are allowed to create/integrate a candidate.
  const writable =
    workItem.role === 'builder' ||
    workItem.role === 'dev_ops' ||
    (workItem.role === 'lead' && (leadPhase === 'implement' || leadPhase === 'post'))
  const context: AgentExecutionContext = {
    command,
    story,
    policy: {
      allowCommit: writable,
      allowDevDbWrite: writable,
      allowControlPlaneWrite: true,
    },
    capabilities: profileConfig.capabilities,
    executionEnvironment: workItem.executionEnvironment ?? null,
    storyRunId: '',
  }

  if (workItem.executionEnvironment) {
    const { assertExecutionTargetSafe } = await import('../lib/execution-target')
    assertExecutionTargetSafe(workItem.executionEnvironment as never)
  }

  let executionWorkspace: AgentExecutionWorkspace | null = null
  if (deps.workspaces) {
    // Assay and Lead POST must start from the newest candidate commit. PRE,
    // SOLO implementation, Architect and Smith start from the approved base.
    const needsCandidateBase =
      isAssayTerminalRole(workItem.role) ||
      (workItem.role === 'lead' && leadPhase === 'post')
    const candidateSha = needsCandidateBase
      ? smithCandidateSha(await deps.runs.listForStory(workItem.storyId))
      : null
    if (needsCandidateBase && !candidateSha) {
      throw new Error(
        `${workItem.role === 'lead' ? 'Lead POST' : 'Assay'} requires an exact candidate commit; refusing to provision from main.`,
      )
    }
    const baseRef = candidateSha ?? deps.workspaces.baseRef
    const ws = await deps.workspaces.provision({
      storyId: workItem.storyId,
      workerId: deps.workspaces.workerId,
      baseRef,
      // One Forge engine invocation owns one serial story workspace. The old
      // workItem.id key created a new branch/worktree for every role transition.
      // SPLIT is currently disabled; when activated, only the explicit split
      // path may call resolveWorkspaceRunId(executionId, splitChildId).
      runId: resolveWorkspaceRunId(deps.workspaces.executionId),
      ...(deps.workspaces.worktreesRoot
        ? { worktreesRoot: deps.workspaces.worktreesRoot }
        : {}),
    })
    if (workItem.executionEnvironment) {
      const { verifyWorkspaceEnvFile } = await import('../lib/execution-target')
      verifyWorkspaceEnvFile(
        ws.worktreePath,
        workItem.executionEnvironment as never,
      )
    }
    executionWorkspace = {
      branchName: ws.branchName,
      worktreePath: ws.worktreePath,
      baseRef: ws.baseRef,
      baseCommit: ws.baseCommit,
      runId: ws.runId,
    }
  }
  const finalContext: AgentExecutionContext =
    executionWorkspace !== null ? { ...context, executionWorkspace } : context

  const evidence = await adapter.execute(command, finalContext)
  return {
    workItemId: workItem.id,
    storyId: workItem.storyId,
    role: command.role,
    modelProfile,
    runtimeAdapter: adapter.runtimeAdapterId,
    evidence,
  }
}

export async function invokeNextAgentCommand(
  workerId: string,
  deps: AgentInvokerDeps,
): Promise<InvokerResult | null> {
  const claim = await claimNextAgentCommand(workerId, deps)
  if (!claim) return null
  return executeClaimedAgentCommand(workerId, claim, deps)
}
