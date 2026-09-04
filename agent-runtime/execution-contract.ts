// ---------------------------------------------------------------------------
// Execution Contract Gate (ENG-FORGE-V4-08) — one explicit, testable
// validation seam before a Smith launch is allowed to begin.
//
// The gate validates the FULLY MERGED story packet (Story Board truth + git
// packet truth) plus the RESOLVED runtime assignment. It fails closed with
// concrete reasons and NEVER invents a missing value, never falls back to a
// different player/field/profile, and never silently fills an empty brief,
// acceptance criteria, or execution target.
//
// Assay planning is intentionally NOT a Smith-launch prerequisite. Smith may
// finish useful code and produce a candidate before an Assay recipe exists.
// The Smith→Assay handoff owns the Assay-plan gate so a missing plan preserves
// the completed candidate and holds the story for verification planning rather
// than discarding useful work at launch time.
//
// The result is a small PROVIDER-NEUTRAL contract (`ok + reasons/code`).
// Hydration/enqueue boundaries and the launch boundary reuse the same
// validator, so orchestration logic never embeds DeepSeek/Warp/OpenClaw model
// names: it only ever sees logical profiles, Forge fields, capabilities, and
// the four execution targets.
//
// Smith remains the only role that may retain a commit; Assay stays read-only
// and owns acceptance; a work item becoming Done never implies Story Complete.
// Existing V3 defenses are kept in place — this gate strengthens them.
// ---------------------------------------------------------------------------

import type { AgentCapability } from './capabilities'
import { DEFAULT_LANES } from './lanes'
import type { AdapterReadiness } from './readiness'
import type { StoryPacketFields } from './story-session'
import { EXECUTION_ENVIRONMENTS } from '../lib/execution-target'

/** Every fail-closed condition the Smith launch gate can name. */
export type ExecutionContractCheck =
  | 'missing-architect-brief'
  | 'missing-acceptance-criteria'
  | 'missing-execution-target'
  | 'invalid-execution-target'
  | 'profile-unregistered'
  | 'adapter-not-ready'
  | 'field-not-ready'
  | 'insufficient-capabilities'

/** One concrete rejection item; `message` names the failing condition. */
export interface ExecutionContractReason {
  code: ExecutionContractCheck
  message: string
}

export type ExecutionContractResult =
  | { ok: true }
  | { ok: false; code: ExecutionContractCheck; reasons: ExecutionContractReason[] }

/**
 * Minimal provider-neutral runtime boundary the gate needs. Matches the live
 * AgentRuntimeRegistry structurally; tests supply a deterministic fake.
 */
export interface ExecutionContractRegistry {
  hasProfile(profile: string): boolean
  inspectProfileReadiness(profile: string): AdapterReadiness
  resolveProfile(profile: string): { capabilities: AgentCapability[] }
}

export interface ExecutionContractSubject {
  /** Fully merged story packet truth (Story Board + git packet). */
  story: StoryPacketFields
  /** Explicit execution target on the work envelope (DEV|PROD|TEST|LOCAL). */
  executionTarget?: string | null
  /** Logical profile selected for the launch — never inferred/substituted. */
  modelProfile?: string | null
  /** Runtime boundary: profile registration + V4-07 adapter readiness. */
  registry: ExecutionContractRegistry
  /** Assigned Field truth (Forge position assignment; e.g. smith -> local). */
  field?: { id: string; ready: boolean } | null
  /**
   * Capabilities the launch requires. Defaults to the live Smith lane binding
   * so the gate tracks lane-policy changes instead of a stale copy.
   */
  requiredCapabilities?: AgentCapability[]
}

function present(value: string | null | undefined): boolean {
  return Boolean(value && value.trim().length > 0)
}

/**
 * Validate a Smith launch contract. Returns `{ ok: true }` only when every
 * launch-time packet/runtime requirement holds; otherwise `{ ok: false }` with
 * a first-failure `code` plus one reason per violated condition. Assay commands
 * are deliberately excluded here and enforced at Smith→Assay handoff.
 */
export function validateExecutionContract(
  input: ExecutionContractSubject,
): ExecutionContractResult {
  const story = input.story ?? {}
  const reasons: ExecutionContractReason[] = []

  // 1. Non-empty Architect brief.
  if (!present(story.architectBrief)) {
    reasons.push({
      code: 'missing-architect-brief',
      message:
        'Architect brief is empty — a Smith launch requires the Architect plan on the merged story packet',
    })
  }

  // 2. Non-empty acceptance criteria.
  if (!present(story.acceptanceCriteria)) {
    reasons.push({
      code: 'missing-acceptance-criteria',
      message:
        'Acceptance criteria are empty — a Smith launch requires non-empty acceptance criteria on the merged story packet',
    })
  }

  // 3. Explicit execution target on the work envelope (DEV|PROD|TEST|LOCAL).
  const target = String(input.executionTarget ?? '').trim()
  if (target === '') {
    reasons.push({
      code: 'missing-execution-target',
      message:
        'work envelope carries no execution target — an explicit DEV|PROD|TEST|LOCAL is required (never inferred)',
    })
  } else if (!(EXECUTION_ENVIRONMENTS as readonly string[]).includes(target.toUpperCase())) {
    reasons.push({
      code: 'invalid-execution-target',
      message: `execution target '${target}' is not one of DEV|PROD|TEST|LOCAL`,
    })
  }

  // 4. Assigned logical profile exists (never invented, never substituted).
  const profile = String(input.modelProfile ?? '').trim()
  if (profile === '') {
    reasons.push({
      code: 'profile-unregistered',
      message: 'no logical profile is assigned to the Smith launch',
    })
  } else if (!input.registry.hasProfile(profile)) {
    reasons.push({
      code: 'profile-unregistered',
      message: `profile '${profile}' is not registered — do not invent or fall back to another profile`,
    })
  } else {
    // 5. Selected adapter/player runtime is ready under the V4-07 readiness gate.
    const readiness = input.registry.inspectProfileReadiness(profile)
    if (!readiness.ready) {
      reasons.push({
        code: 'adapter-not-ready',
        message: `runtime for profile '${profile}' is not ready: ${readiness.reason}`,
      })
    }

    // 7. Smith-required capabilities are satisfied by the selected runtime/profile.
    const required =
      input.requiredCapabilities ?? DEFAULT_LANES.smith.requiredCapabilities
    const config = input.registry.resolveProfile(profile)
    const missing = required.filter((capability) => !config.capabilities.includes(capability))
    if (missing.length > 0) {
      reasons.push({
        code: 'insufficient-capabilities',
        message: `profile '${profile}' does not satisfy Smith-required capabilities: ${missing.join(', ')}`,
      })
    }
  }

  // 6. Assigned Field is ready (never a fallback field).
  const field = input.field
  if (!field) {
    reasons.push({
      code: 'field-not-ready',
      message: 'no assigned Field is resolved for the Smith launch',
    })
  } else if (!field.ready) {
    reasons.push({
      code: 'field-not-ready',
      message: `assigned Field '${field.id}' is not ready — a Smith launch must not fall back to another field`,
    })
  }

  if (reasons.length === 0) return { ok: true }
  return { ok: false, code: reasons[0].code, reasons }
}

/** Compact single-line evidence for logs/error_text; null when the gate passes. */
export function executionContractFailureText(
  result: ExecutionContractResult,
): string | null {
  if (result.ok) return null
  return result.reasons.map((reason) => `${reason.code}: ${reason.message}`).join(' | ')
}
