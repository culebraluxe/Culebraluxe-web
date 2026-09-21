// ---------------------------------------------------------------------------
// ENG-FORGE-HARDEN-02 — first-class failure classification + deterministic
// routing.
//
// A failure is not inherently a Smith failure. Forge must classify a failure by
// CAUSE and route it to the role with authority to repair that cause, never
// blindly retrying the lane that happened to observe it. Routing is derived
// from existing Forge role/lane semantics (agent-runtime/lanes.ts); it is not
// invented here.
//
// Canonical bounded taxonomy (no uncontrolled free-text types). UNKNOWN always
// fails safely (HOLD). Routing is deterministic and bounded by attempt count.
// ---------------------------------------------------------------------------

export type ForgeFailureClass =
  | 'MISSING_CONTEXT'
  | 'BAD_IMPLEMENTATION'
  | 'BAD_ARCHITECTURE'
  | 'BAD_TOOL_CONTRACT'
  | 'ENVIRONMENT_FAILURE'
  | 'MISSING_GUARDRAIL'
  | 'WEAK_TEST'
  | 'DEPENDENCY_FAILURE'
  | 'DEPLOYMENT_FAILURE'
  | 'UNKNOWN'

export const FORGE_FAILURE_CLASSES: readonly ForgeFailureClass[] = [
  'MISSING_CONTEXT',
  'BAD_IMPLEMENTATION',
  'BAD_ARCHITECTURE',
  'BAD_TOOL_CONTRACT',
  'ENVIRONMENT_FAILURE',
  'MISSING_GUARDRAIL',
  'WEAK_TEST',
  'DEPENDENCY_FAILURE',
  'DEPLOYMENT_FAILURE',
  'UNKNOWN',
]

export function isForgeFailureClass(v: unknown): v is ForgeFailureClass {
  return typeof v === 'string' && (FORGE_FAILURE_CLASSES as readonly string[]).includes(v)
}

export type ClassifySignals = {
  /** A candidate class from structured evidence (never free-form prose). */
  candidate?: string
  lane?: string
  /** Narrow, unambiguous external signal used only when no candidate is given. */
  observed?:
    | 'deploy'
    | 'env'
    | 'missing-context'
    | 'arch'
    | 'tool'
    | 'dependency'
    | 'guardrail'
    | undefined
  detail?: string
}

export type ForgeFailureClassification = {
  class: ForgeFailureClass
  reason: string
  /** True when no canonical class could be established. */
  unknown: boolean
}

// Deterministic inference only for signals that are unambiguous. Anything else
// without an explicit canonical candidate resolves to UNKNOWN -> safe HOLD.
const SIGNAL_TO_CLASS: Partial<Record<NonNullable<ClassifySignals['observed']>, ForgeFailureClass>> = {
  deploy: 'DEPLOYMENT_FAILURE',
  env: 'ENVIRONMENT_FAILURE',
  'missing-context': 'MISSING_CONTEXT',
  arch: 'BAD_ARCHITECTURE',
  tool: 'BAD_TOOL_CONTRACT',
  dependency: 'DEPENDENCY_FAILURE',
  guardrail: 'MISSING_GUARDRAIL',
}

export function classifyFailure(input: ClassifySignals): ForgeFailureClassification {
  if (isForgeFailureClass(input.candidate)) {
    return { class: input.candidate, reason: input.detail ?? input.candidate, unknown: false }
  }
  if (input.observed) {
    const cls = SIGNAL_TO_CLASS[input.observed]
    if (cls) return { class: cls, reason: input.detail ?? cls, unknown: false }
  }
  return { class: 'UNKNOWN', reason: input.detail ?? 'unclassifiable failure', unknown: true }
}

// ---------------------------------------------------------------------------
// Deterministic routing: FAIL -> CLASSIFY -> ROUTE. Owner derived from existing
// role semantics; never invented by an agent. UNKNOWN and DEPENDENCY_FAILURE
// fail safely to HOLD. Retries are bounded by maxAttempts.
// ---------------------------------------------------------------------------

export type RepairOwner = 'smith' | 'scout' | 'architect' | 'lead' | 'qa' | 'dev_ops'

export type ForgeFailureRouting =
  | { action: 'repair'; owner: RepairOwner; attempts: number }
  | { action: 'hold'; reason: string; attempts: number }

const CLASS_TO_ROUTE: Partial<Record<ForgeFailureClass, RepairOwner | 'hold'>> = {
  MISSING_CONTEXT: 'scout',
  BAD_IMPLEMENTATION: 'smith',
  BAD_ARCHITECTURE: 'architect',
  BAD_TOOL_CONTRACT: 'lead',
  ENVIRONMENT_FAILURE: 'dev_ops',
  MISSING_GUARDRAIL: 'lead',
  WEAK_TEST: 'qa',
  DEPENDENCY_FAILURE: 'hold',
  DEPLOYMENT_FAILURE: 'dev_ops',
  UNKNOWN: 'hold',
}

/**
 * Deterministic, bounded routing. `attempts` is the 1-based attempt about to be
 * authorized; when it already meets/exceeds maxAttempts the routing HOLDs
 * (retry exhaustion) rather than authorizing another cycle.
 */
export function routeFailure(input: {
  class: ForgeFailureClass
  attempts: number
  maxAttempts: number
}): ForgeFailureRouting {
  if (input.attempts >= input.maxAttempts) {
    return {
      action: 'hold',
      reason: `retry budget exhausted (${input.attempts}/${input.maxAttempts}) for ${input.class}`,
      attempts: input.attempts,
    }
  }
  const route = CLASS_TO_ROUTE[input.class]
  if (route === 'hold' || !route) {
    return { action: 'hold', reason: `${input.class} requires operator/Lead intervention`, attempts: input.attempts }
  }
  return { action: 'repair', owner: route, attempts: input.attempts }
}

