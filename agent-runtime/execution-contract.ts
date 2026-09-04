// ---------------------------------------------------------------------------
// Forge V6 deterministic Smith preflight.
//
// Before token spend, the fully merged story packet and resolved runtime must
// already describe a build that Forge knows how to verify. Missing architecture,
// acceptance criteria, Assay commands, execution target, runtime readiness, or
// capability is a preflight failure — Smith never launches on hope.
// ---------------------------------------------------------------------------

import type { AgentCapability } from './capabilities'
import { DEFAULT_LANES } from './lanes'
import type { AdapterReadiness } from './readiness'
import type { StoryPacketFields } from './story-session'
import { EXECUTION_ENVIRONMENTS } from '../lib/execution-target'
import { parseAssayCommands } from './assay-plan'

/** Every fail-closed condition the Smith launch gate can name. */
export type ExecutionContractCheck =
  | 'missing-architect-brief'
  | 'missing-acceptance-criteria'
  | 'missing-assay-plan'
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

export interface ExecutionContractRegistry {
  hasProfile(profile: string): boolean
  inspectProfileReadiness(profile: string): AdapterReadiness
  resolveProfile(profile: string): { capabilities: AgentCapability[] }
}

export interface ExecutionContractSubject {
  story: StoryPacketFields
  executionTarget?: string | null
  modelProfile?: string | null
  registry: ExecutionContractRegistry
  field?: { id: string; ready: boolean } | null
  requiredCapabilities?: AgentCapability[]
}

function present(value: string | null | undefined): boolean {
  return Boolean(value && value.trim().length > 0)
}

/**
 * Validate the entire deterministic contract required before Smith can launch.
 * The Assay recipe is included in V6: if Forge cannot state how the candidate
 * will be verified before build, it does not spend the builder call.
 */
export function validateExecutionContract(
  input: ExecutionContractSubject,
): ExecutionContractResult {
  const story = input.story ?? {}
  const reasons: ExecutionContractReason[] = []

  if (!present(story.architectBrief)) {
    reasons.push({
      code: 'missing-architect-brief',
      message:
        'Architect brief is empty — Smith requires the authoritative plan before launch',
    })
  }

  if (!present(story.acceptanceCriteria)) {
    reasons.push({
      code: 'missing-acceptance-criteria',
      message:
        'Acceptance criteria are empty — Smith requires a verifiable done condition before launch',
    })
  }

  if (parseAssayCommands(story.assayCommands).length === 0) {
    reasons.push({
      code: 'missing-assay-plan',
      message:
        'Assay commands are empty — Forge V6 does not spend Smith until the candidate has an explicit verification recipe',
    })
  }

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
    const readiness = input.registry.inspectProfileReadiness(profile)
    if (!readiness.ready) {
      reasons.push({
        code: 'adapter-not-ready',
        message: `runtime for profile '${profile}' is not ready: ${readiness.reason}`,
      })
    }

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

export function executionContractFailureText(
  result: ExecutionContractResult,
): string | null {
  if (result.ok) return null
  return result.reasons.map((reason) => `${reason.code}: ${reason.message}`).join(' | ')
}
