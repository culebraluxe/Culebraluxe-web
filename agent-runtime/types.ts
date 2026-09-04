// ---------------------------------------------------------------------------
// Agent Runtime domain types (ENG-18 core slice).
// ---------------------------------------------------------------------------

import type { StoryboardStory } from '../db/storyboard'
import type { AgentCapability } from './capabilities'
import type { AssayEvidence } from './assay-evidence'
import type { LaneId } from './lanes'
import type { LeadRunPhase } from './lead-decision'
import type { StoryPacketFields } from './story-session'

/** Logical agent roles (extensible — not a closed enum). */
export type AgentRole =
  | 'architect'
  | 'lead'
  | 'builder'
  | 'reviewer'
  | 'verifier'
  | (string & {})

/** Logical model profile — a capability/quality label, never a vendor model id. */
export type ModelProfile = string

export type AgentRuntimeSelection = {
  playerId: string
  providerId: string
  modelId: string
  harnessId: string
  fieldId: string
}

export type AgentCommandState =
  | 'Ready'
  | 'Claimed'
  | 'Running'
  | 'Paused'
  | 'Done'
  | 'Error'
  | 'Cancelled'

export type AdapterRuntimeStatus =
  | 'idle'
  | 'starting'
  | 'running'
  | 'paused'
  | 'success'
  | 'failed'
  | 'cancelled'

/** Durable Agent Work Command — WHAT work is requested and WHO/HOW was frozen for it. */
export interface AgentWorkCommand {
  workItemId: string
  storyId: string
  role: AgentRole
  lane: LaneId
  runPhase: LeadRunPhase | null
  modelProfile: ModelProfile
  runtimeSelection: AgentRuntimeSelection
  specialInstructions: string | null
  candidateShas: string[]
  parallelGroupId: string | null
  parallelSlot: number | null
  parallelSize: number | null
  splitAssignment: string | null
  priority: number
  state: AgentCommandState
  claimedBy: string | null
  claimedAt: string | null
  startedAt: string | null
  finishedAt: string | null
  storyRunId: string | null
  errorText: string | null
  runtimeAdapter: string | null
  externalRunId: string | null
  attempts: number
  maxAttempts: number
  executionEnvironment?: string | null
  createdAt: string
  updatedAt: string
}

/** Execution context handed to a runtime adapter for ONE attempt. */
export interface AgentExecutionContext {
  command: AgentWorkCommand
  /** Run-bound execution view; architecture/acceptance/test-plan fields are frozen on the Run. */
  story: StoryboardStory & StoryPacketFields
  policy: AgentExecutionPolicy
  capabilities: AgentCapability[]
  /**
   * Convenience copy for active execution. The durable command is authoritative;
   * pause/resume/cancel helper contexts may omit this copy and use command.runtimeSelection.
   */
  runtimeSelection?: AgentRuntimeSelection
  executionEnvironment?: string | null
  executionWorkspace?: AgentExecutionWorkspace | null
  storyRunId: string
}

export interface AgentExecutionWorkspace {
  branchName: string
  worktreePath: string
  baseRef: string
  baseCommit: string
  runId: string
}

export interface AgentExecutionPolicy {
  allowCommit: boolean
  allowDevDbWrite: boolean
  allowControlPlaneWrite: boolean
}

export interface AgentRunEvidence {
  resultStatus: string
  completion: number
  notes: string
  testsSummary: string | null
  commitHash: string | null
  assayEvidence?: AssayEvidence | null
  runtimeAdapter: string | null
  modelProfile: ModelProfile | null
  externalRunId: string | null
  executionEnvironment?: string | null
  startedAt: string
  endedAt: string | null
}

export type AgentProgressStep =
  | 'claimed'
  | 'loading_context'
  | 'executing'
  | 'running_tests'
  | 'collecting_evidence'
  | 'terminalizing'

export interface AgentProgressUpdate {
  step?: AgentProgressStep | string
  completion?: number
  note?: string
  testsSummary?: string | null
}
