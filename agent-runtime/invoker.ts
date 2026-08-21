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
  /** Default model profile used when a command carries none. */
  defaultProfile?: string
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

  // Resolve the logical profile (default when absent).
  const modelProfile = workItem.modelProfile ?? deps.defaultProfile ?? 'builder-flash'
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
    storyRunId: '',
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
