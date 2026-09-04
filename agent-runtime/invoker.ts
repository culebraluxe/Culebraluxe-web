// ---------------------------------------------------------------------------
// Poller / Invoker — V6.1 executes the durable typed work assignment.
// ---------------------------------------------------------------------------

import { AgentRuntimeRegistry } from './registry'
import { rejectAgentWorkConfiguration, validateAgentWorkLaunchConfig } from '../db/agent-work'
import type {
  AgentExecutionContext,
  AgentExecutionWorkspace,
  AgentRunEvidence,
  AgentWorkCommand,
} from './types'
import type { AgentRunRepository, AgentWorkRepository } from './repositories'
import type { ForgeAgentWorkClaim } from '../db/agent-work-v61'
import type { AgentCapability } from './capabilities'
import { executionContractFailureText, validateExecutionContract } from './execution-contract'
import { storyFieldsFromBoardAndGit } from './orchestrate'
import { DEFAULT_LANES } from './lanes'
import { FORGE_FIELDS, type ForgeHarnessId } from './team'
import { adapterIdForHarness } from './factory'
import { leadRunPhaseFromInstructions } from './lead-decision'
import { resolveApprovedBaseRef } from '../lib/worker-workspace'
import { provisionOrRecoverWorkerWorkspace } from '../lib/worker-workspace/recovering-provisioner'
import type { WorkerWorkspace, WorkerWorkspaceSpec } from '../lib/worker-workspace/types'

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

export function buildAgentInvokerWorkspaces(
  workerId: string,
  env: NodeJS.ProcessEnv = process.env,
): AgentInvokerWorkspaces | undefined {
  if ((env.AGENT_WORKSPACE_DISABLED ?? '0').trim() === '1') return undefined
  const baseRef = resolveApprovedBaseRef(env)
  const worktreesRoot = (env.AGENT_WORKSPACE_WORKTREES_ROOT ?? '').trim() || undefined
  return {
    workerId,
    baseRef,
    ...(worktreesRoot ? { worktreesRoot } : {}),
    provision: provisionOrRecoverWorkerWorkspace,
  }
}

export async function claimNextAgentCommand(
  workerId: string,
  deps: AgentInvokerDeps,
): Promise<ForgeAgentWorkClaim | null> {
  return deps.work.claimNext(workerId)
}

export async function executeClaimedAgentCommand(
  _workerId: string,
  claim: ForgeAgentWorkClaim,
  deps: AgentInvokerDeps,
): Promise<InvokerResult> {
  const workItem = claim.workItem
  const story = claim.story
  const leadPhase = workItem.runPhase ??
    (workItem.role === 'lead' ? leadRunPhaseFromInstructions(workItem.specialInstructions) : null)

  const launchError = validateAgentWorkLaunchConfig(workItem)
  if (launchError) {
    await rejectAgentWorkConfiguration(workItem.id, launchError)
    throw new Error(`launch guard: ${launchError}`)
  }
  if (!workItem.lane || !workItem.harnessId || !workItem.playerId || !workItem.providerId || !workItem.modelId || !workItem.fieldId) {
    const detail = 'typed Forge routing is incomplete (lane/player/provider/model/harness/field required)'
    await rejectAgentWorkConfiguration(workItem.id, detail)
    throw new Error(`launch guard: ${detail}`)
  }

  const lane = workItem.lane
  const adapterId = adapterIdForHarness(workItem.harnessId as ForgeHarnessId)
  const adapterCapabilities = deps.registry.adapterCapabilities(adapterId)
  const laneCapabilities = DEFAULT_LANES[lane].requiredCapabilities
  const required = [...new Set([...(deps.requiredCapabilities ?? []), ...laneCapabilities])]
  const missing = required.filter((capability) => !adapterCapabilities.includes(capability))
  if (missing.length > 0) {
    throw new Error(
      `frozen harness '${workItem.harnessId}' adapter '${adapterId}' lacks ${missing.join(',')} required by lane '${lane}'`,
    )
  }

  if (deps.enforceExecutionContract && lane === 'smith') {
    const merged = storyFieldsFromBoardAndGit(story, story.id)
    const field = FORGE_FIELDS[workItem.fieldId as keyof typeof FORGE_FIELDS]
    const contract = validateExecutionContract({
      story: merged,
      executionTarget: workItem.executionEnvironment,
      modelProfile: workItem.modelProfile,
      registry: deps.registry,
      field: { id: workItem.fieldId, ready: Boolean(field?.ready) },
    })
    if (!contract.ok) {
      const evidence = `execution contract gate: ${executionContractFailureText(contract) ?? contract.code}`
      await rejectAgentWorkConfiguration(workItem.id, evidence)
      throw new Error(evidence)
    }
  }

  const adapter = deps.registry.resolveAdapterById(adapterId, { work: deps.work, runs: deps.runs })
  await deps.work.setRuntime(workItem.id, { runtimeAdapter: adapter.runtimeAdapterId })

  const runtimeSelection = {
    playerId: workItem.playerId,
    providerId: workItem.providerId,
    modelId: workItem.modelId,
    harnessId: workItem.harnessId,
    fieldId: workItem.fieldId,
  }
  const command: AgentWorkCommand = {
    workItemId: workItem.id,
    storyId: workItem.storyId,
    role: (workItem.role ?? 'builder') as AgentWorkCommand['role'],
    lane,
    runPhase: leadPhase,
    modelProfile: workItem.modelProfile as string,
    runtimeSelection,
    specialInstructions: workItem.specialInstructions ?? null,
    candidateShas: workItem.candidateShas,
    parallelGroupId: workItem.parallelGroupId,
    parallelSlot: workItem.parallelSlot,
    parallelSize: workItem.parallelSize,
    splitAssignment: workItem.splitAssignment,
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

  const writable = lane === 'smith' || (lane === 'lead' && (leadPhase === 'implement' || leadPhase === 'post'))
  const context: AgentExecutionContext = {
    command,
    story,
    policy: {
      allowCommit: writable,
      allowDevDbWrite: writable,
      allowControlPlaneWrite: true,
    },
    capabilities: adapterCapabilities,
    runtimeSelection,
    executionEnvironment: workItem.executionEnvironment ?? null,
    storyRunId: '',
  }

  if (workItem.executionEnvironment) {
    const { assertExecutionTargetSafe } = await import('../lib/execution-target')
    assertExecutionTargetSafe(workItem.executionEnvironment as never)
  }

  let executionWorkspace: AgentExecutionWorkspace | null = null
  if (deps.workspaces) {
    const needsCandidateBase = lane === 'assay' || lane === 'inspector' || (lane === 'lead' && leadPhase === 'post')
    const candidateSha = needsCandidateBase ? workItem.candidateShas[0] ?? null : null
    if (needsCandidateBase && !candidateSha) {
      throw new Error(`${lane}${leadPhase ? `/${leadPhase}` : ''} requires a typed candidate commit; refusing to provision from main.`)
    }
    const baseRef = candidateSha ?? deps.workspaces.baseRef
    const ws = await deps.workspaces.provision({
      storyId: workItem.storyId,
      workerId: deps.workspaces.workerId,
      baseRef,
      runId: workItem.id,
      ...(deps.workspaces.worktreesRoot ? { worktreesRoot: deps.workspaces.worktreesRoot } : {}),
    })
    if (workItem.executionEnvironment) {
      const { verifyWorkspaceEnvFile } = await import('../lib/execution-target')
      verifyWorkspaceEnvFile(ws.worktreePath, workItem.executionEnvironment as never)
    }
    executionWorkspace = {
      branchName: ws.branchName,
      worktreePath: ws.worktreePath,
      baseRef: ws.baseRef,
      baseCommit: ws.baseCommit,
      runId: ws.runId,
    }
  }

  const finalContext: AgentExecutionContext = executionWorkspace ? { ...context, executionWorkspace } : context
  const evidence = await adapter.execute(command, finalContext)
  return {
    workItemId: workItem.id,
    storyId: workItem.storyId,
    role: command.role,
    modelProfile: command.modelProfile,
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
