// ---------------------------------------------------------------------------
// Poller / Invoker (ENG-18) — the boring parent wrapper.
//
// Responsibilities (ALL vendor-neutral):
//   1. find next ELIGIBLE work and atomically claim it
//   2. load canonical Story Board context by story_id
//   3. construct the AgentWorkCommand / AgentExecutionContext
//   4. resolve the runtime adapter from the logical model profile
//   5. call adapter execution
//   6. maintain heartbeat/lease while active
//   7. normalize result
//   8. persist Story Board run evidence
//   9. terminalize the work item
//
// It knows NOTHING about how DeepSeek reasons or how a local model works.
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
  provisionWorkerWorkspace,
  resolveApprovedBaseRef,
} from '../lib/worker-workspace'
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

/**
 * ENG-21 — isolated worker workspace provisioning config. Absent = the legacy
 * shared-checkout execution path (byte-for-byte unchanged). Present = the
 * worker executes in its OWN branch + worktree from an EXPLICIT approved base
 * ref; the primary checkout is never a worker scratch directory.
 */
export interface AgentInvokerWorkspaces {
  /** The worker identity that owns the workspace branch. */
  workerId: string
  /** EXPLICIT approved integration base ref (branch/tag/commit) — never
   * HEAD-derived; pinned to a fixed commit at provision time. */
  baseRef: string
  /** Optional directory where worker worktrees live (must be OUTSIDE the
   *  primary checkout). Defaults to `../Culebraluxe-worktrees` next to it. */
  worktreesRoot?: string
  /** Provision branch + worktree; returns the isolated workspace. */
  provision: (spec: WorkerWorkspaceSpec) => Promise<WorkerWorkspace>
}

export interface AgentInvokerDeps {
  work: AgentWorkRepository
  runs: AgentRunRepository
  registry: AgentRuntimeRegistry
  /** Capability gate: if the profile's adapter lacks a required capability, the
   * command is NOT eligible and is left Ready (deterministic eligibility). */
  requiredCapabilities?: AgentCapability[]
  /** Optional isolated-worker workspace provisioning (ENG-21). */
  workspaces?: AgentInvokerWorkspaces
}

/**
 * Build the optional isolated-worker workspace dep from the operator
 * environment (ENG-21). Default: workspace execution is ENABLED with the
 * explicit approved base ref (`AGENT_WORKSPACE_BASE_REF`, else the repo's
 * canonical `main` branch) — the primary checkout is never a worker scratch
 * directory. `AGENT_WORKSPACE_DISABLED=1` restores the legacy shared-checkout
 * path explicitly (documented escape hatch). `AGENT_WORKSPACE_WORKTREES_ROOT`
 * relocates the worktree directory outside the repo.
 */
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
    provision: provisionWorkerWorkspace,
  }
}

/**
 * Phase 1 — atomically claim the next eligible work item (single-worker rule).
 * Returns null when there is no Ready work or another item is already active.
 * Used directly by the scheduler/poller so it can invoke the runtime on an
 * already-claimed command without a story-specific launch command.
 */
export async function claimNextAgentCommand(
  workerId: string,
  deps: AgentInvokerDeps,
): Promise<AgentWorkClaim | null> {
  return deps.work.claimNext(workerId)
}

/**
 * Phase 2 — drive an ALREADY-CLAIMED command through the runtime:
 *   - hard launch guard (missing/invalid envelope terminalized, slot released)
 *   - resolve adapter from the persisted model profile (no silent default)
 *   - capability gate
 *   - persist runtime_adapter before the Running transition
 *   - execution-target fail-fast guard
 *   - adapter.execute (heartbeat / session / evidence / finalization unchanged)
 * This is the SAME code path `invokeNextAgentCommand` uses — no duplicate
 * execution mechanism.
 */
export async function executeClaimedAgentCommand(
  workerId: string,
  claim: AgentWorkClaim,
  deps: AgentInvokerDeps,
): Promise<InvokerResult> {
  const workItem = claim.workItem
  const story = claim.story

  // HARD LAUNCH GUARD (ENG-20A): the durable command must carry the execution
  // configuration required to launch a runtime. A work item cannot become
  // Running merely because it was claimed — missing/invalid configuration is
  // terminalized (work Error + story Hold + global slot released) and the
  // invoker aborts BEFORE any runtime work begins. No silent default
  // substitutes operator intent.
  const launchError = validateAgentWorkLaunchConfig(workItem)
  if (launchError) {
    await rejectAgentWorkConfiguration(workItem.id, launchError)
    throw new Error(`launch guard: ${launchError}`)
  }

  // The guard guarantees the durable command carries a logical model profile;
  // the invoker consumes ONLY persisted configuration (no in-memory default).
  const modelProfile = workItem.modelProfile as string
  const profileConfig = deps.registry.resolveProfile(modelProfile)

  // Deterministic eligibility: capability gate.
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

  // Persist the resolved runtime adapter BEFORE the Running transition
  // (no later than the pre-Running launch boundary) so the durable command
  // always answers "which adapter will execute this".
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

  const context: AgentExecutionContext = {
    command,
    story,
    policy: {
      allowCommit: true,
      allowDevDbWrite: true,
      allowControlPlaneWrite: true,
    },
    capabilities: profileConfig.capabilities,
    executionEnvironment: workItem.executionEnvironment ?? null,
    storyRunId: '',
  }

  // FAIL-FAST (ENG-20): before any database-affecting SDLC work begins,
  // verify the application/domain DB configuration matches the command's
  // intended execution target. A DEV command that would resolve to the
  // production application DB (including through a generic DATABASE_URL
  // fallback) is refused here — BEFORE the external runtime is started.
  if (workItem.executionEnvironment) {
    const { assertExecutionTargetSafe } = await import('../lib/execution-target')
    assertExecutionTargetSafe(workItem.executionEnvironment as never)
  }

  // ENG-21 — isolated worker workspace (branch + worktree). Absent config keeps
  // the legacy shared-checkout path byte-for-byte. When configured, the worker
  // executes in its OWN worktree from an EXPLICIT approved base ref; the
  // primary checkout is never a worker scratch directory. The environment
  // boundary guard then runs against the isolated workspace's .env.local so a
  // DEV-intended command can never resolve to the PROD application DB through
  // the worker's shared local configuration.
  let executionWorkspace: AgentExecutionWorkspace | null = null
  if (deps.workspaces) {
    const ws = await deps.workspaces.provision({
      storyId: workItem.storyId,
      workerId: deps.workspaces.workerId,
      baseRef: deps.workspaces.baseRef,
      runId: workItem.id,
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

/** Full poll cycle: atomically claim the next item, then execute it through the
 * runtime. Used by the manual/debug driver; the scheduler uses the two phases
 * directly on its already-claimed command. */
export async function invokeNextAgentCommand(
  workerId: string,
  deps: AgentInvokerDeps,
): Promise<InvokerResult | null> {
  const claim = await claimNextAgentCommand(workerId, deps)
  if (!claim) return null
  return executeClaimedAgentCommand(workerId, claim, deps)
}
