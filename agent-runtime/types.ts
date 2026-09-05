// ---------------------------------------------------------------------------
// Agent Runtime domain types (ENG-18 core slice).
//
// DOMAIN-NEUTRAL. No vendor nouns are permitted in this module:
//   - no DeepSeek model ids / session ids
//   - no OpenHands states
//   - no MLX/model names
//   - no provider API details
//
// The SDLC command model is intentionally minimal: storyId + role +
// modelProfile (LOGICAL) + optional specialInstructions. The authoritative
// story specification is NEVER duplicated here — it is resolved from the
// Story Board at execution time.
// ---------------------------------------------------------------------------

import type { StoryboardStory } from '../db/storyboard'
import type { AgentCapability } from './capabilities'
import type { AssayEvidence } from './assay-evidence'
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

/** Canonical work-item lifecycle states (migration 025 + 028). */
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

export interface AgentWorkCommand {
  workItemId: string
  storyId: string
  role: AgentRole
  modelProfile: ModelProfile
  specialInstructions: string | null
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

export interface AgentExecutionContext {
  command: AgentWorkCommand
  story: StoryboardStory & StoryPacketFields
  policy: AgentExecutionPolicy
  capabilities: AgentCapability[]
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
  releaseEvidence?: {
    kind: 'deployment' | 'production_verification'
    artifactSha: string
    receiptId: string
    success: boolean
  } | null
  /** V11 — typed gate facts. Preferred over JSON-in-notes markers. */
  gateEvidence?: Record<string, unknown> | null
  modelUsed?: string | null
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
