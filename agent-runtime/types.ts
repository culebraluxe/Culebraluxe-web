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
export type AgentRole = 'architect' | 'builder' | 'reviewer' | 'verifier' | (string & {})

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

/**
 * Canonical runtime status vocabulary exposed by adapters. Deliberately
 * generic; the adapter maps vendor statuses into these.
 */
export type AdapterRuntimeStatus =
  | 'idle'
  | 'starting'
  | 'running'
  | 'paused'
  | 'success'
  | 'failed'
  | 'cancelled'

/** Durable Agent Work Command — WHAT work is requested. */
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
  /** Intended execution target (DEV|PROD|TEST|LOCAL) — never inferred from
   * the control-plane database; set explicitly on the durable command. */
  executionEnvironment?: string | null
  createdAt: string
  updatedAt: string
}

/** Execution context handed to a runtime adapter for ONE attempt. */
export interface AgentExecutionContext {
  command: AgentWorkCommand
  /**
   * Run-bound execution view. Stable Story identity metadata may originate on
   * the parent, but architecture/acceptance/test-plan fields are frozen on
   * storyboard_story_run before any agent starts.
   */
  story: StoryboardStory & StoryPacketFields
  /** Resolved runtime policy for this execution. */
  policy: AgentExecutionPolicy
  /** Capabilities the selected runtime advertises/requires. */
  capabilities: AgentCapability[]
  /** Intended execution target (DEV|PROD|TEST|LOCAL) for this attempt. */
  executionEnvironment?: string | null
  /**
   * ENG-21 — isolated worker workspace (branch + worktree) when the invoker
   * provisioned one. Adapters run external processes HERE (cwd = the isolated
   * worktree path) and record branch/worktree/base in run evidence; absent
   * keeps the legacy shared-checkout workspace. Execution infrastructure only.
   */
  executionWorkspace?: AgentExecutionWorkspace | null
  /** The storyboard_story_run id for this attempt (created by the invoker). */
  storyRunId: string
}

/**
 * ENG-21 — one isolated worker execution context (branch + worktree + the
 * approved base commit it was pinned to). Execution infrastructure only; git
 * is the repository's own VCS, never a vendor.
 */
export interface AgentExecutionWorkspace {
  /** `agent/<story>/<run>` — the unique local branch owned by this worker. */
  branchName: string
  /** Absolute path of the isolated worktree (outside the primary checkout). */
  worktreePath: string
  /** The approved base ref as supplied (branch/tag/commit). */
  baseRef: string
  /** The fixed commit the workspace was created from. */
  baseCommit: string
  /** Deterministic run id for this execution (work-item id when dispatched). */
  runId: string
}

export interface AgentExecutionPolicy {
  /** Authorized to create a local git commit (never push). */
  allowCommit: boolean
  /** Authorized to write DEV database / migrations. */
  allowDevDbWrite: boolean
  /** Authorized to write production control-plane (Story Board) state. */
  allowControlPlaneWrite: boolean
}

/**
 * Normalized adapter evidence. Narrative fields remain human-readable. Rich
 * lane-specific evidence may travel in-process here; V6 projects durable truth
 * into generic structured columns on the existing storyboard_story_run row.
 */
export interface AgentRunEvidence {
  resultStatus: string
  completion: number
  notes: string
  testsSummary: string | null
  commitHash: string | null
  /** Deterministic Assay's rich in-process evidence before generic Run projection. */
  assayEvidence?: AssayEvidence | null
  runtimeAdapter: string | null
  modelProfile: ModelProfile | null
  externalRunId: string | null
  /** Execution target this run actually executed against (DEV|PROD|TEST|LOCAL). */
  executionEnvironment?: string | null
  startedAt: string
  endedAt: string | null
}

/** Factual progress markers — never invented percent-complete. */
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
