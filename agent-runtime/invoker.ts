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
} from '../db/agent-work'
import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from './types'
import type { AgentRunRepository, AgentWorkRepository } from './repositories'
import type { AgentCapability } from './capabilities'

export type InvokerResult = {
  workItemId: string
  storyId: string
  role: string
  modelProfile: string
  runtimeAdapter: string
  evidence: AgentRunEvidence
}

export interface AgentInvokerDeps {
  work: AgentWorkRepository
  runs: AgentRunRepository
  registry: AgentRuntimeRegistry
  /** Capability gate: if the profile's adapter lacks a required capability, the
   * command is NOT eligible and is left Ready (deterministic eligibility). */
  requiredCapabilities?: AgentCapability[]
}

export async function invokeNextAgentCommand(
  workerId: string,
  deps: AgentInvokerDeps,
): Promise<InvokerResult | null> {
  const claim = await deps.work.claimNext(workerId)
  if (!claim) return null

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

  const evidence = await adapter.execute(command, context)
  return {
    workItemId: workItem.id,
    storyId: workItem.storyId,
    role: command.role,
    modelProfile,
    runtimeAdapter: adapter.runtimeAdapterId,
    evidence,
  }
}
