import type { DomainEventType } from '../workflow/contracts'

// ---------------------------------------------------------------------------
// CRM-27 — Agreement Execution Predicate (provider-neutral).
//
// "Is this specific issued agreement/document version FULLY EXECUTED?"
//
// This is the single provider-neutral application-owned answer to that
// question. It is deliberately:
//   - NOT provider-specific — BoldSign is evidence, not the definition of
//     execution. Neutral signature-role evidence is passed in; no provider
//     status strings ever appear here or in workflow XML.
//   - version-scoped — evidence is keyed to one immutable issued
//     document/agreement version. A different version is a different evidence
//     set and can never falsely satisfy this one.
//   - idempotent — evaluating a verdict more than once yields the same result;
//     the "became fully executed" transition flips true exactly once.
//   - manual/external-capable — an authorized manual/external execution may
//     satisfy the predicate (agreements executed outside the configured
//     signature provider).
//
// The required-role POLICY is intentionally a seam (stop-sign C): which
// declared signature groups are legally/business REQUIRED execution roles is
// NOT resolved by this code. It defaults to "every declared signature group is
// required" — a clearly NON-AUTHORITATIVE fixture default for tests — and can
// be overridden per template id without touching the predicate.
// ---------------------------------------------------------------------------

/** Neutral DomainEvent: an issued agreement/document version became fully
 *  executed (all required execution roles satisfied, or authorized
 *  manual/external execution). Fired exactly once per version. */
export const AGREEMENT_FULLY_EXECUTED: DomainEventType = 'AGREEMENT_FULLY_EXECUTED'

export type AgreementExecutionEvidence = {
  /** Immutable issued agreement/document version this evidence is about. */
  documentVersion: string
  /** Required execution roles (resolved via the policy seam). */
  requiredRoles: readonly string[]
  /** Roles with satisfied NEUTRAL signature-role evidence (completed requests). */
  satisfiedRoles: readonly string[]
  /** Authorized manual/external execution recorded for this version. */
  manuallyExecuted: boolean
}

export type AgreementExecutionReason =
  | 'all_required_roles_satisfied'
  | 'manual_execution'
  | 'missing_required_roles'

export type AgreementExecutionVerdict = {
  fullyExecuted: boolean
  missingRoles: readonly string[]
  reason: AgreementExecutionReason
}

/**
 * Pure predicate: compute whether the evidence proves full execution of this
 * specific document version.
 *
 *   partial required signatures      -> not fully executed (missingRoles listed)
 *   final required evidence arrives  -> fully executed
 *   authorized manual/external       -> fully executed (satisfies the predicate)
 *
 * Never mutates state. Deterministic for a given evidence input.
 */
export function evaluateAgreementExecution(
  evidence: AgreementExecutionEvidence,
): AgreementExecutionVerdict {
  if (evidence.manuallyExecuted) {
    return {
      fullyExecuted: true,
      missingRoles: [],
      reason: 'manual_execution',
    }
  }
  const satisfied = new Set(evidence.satisfiedRoles)
  const missingRoles = evidence.requiredRoles.filter((role) => !satisfied.has(role))
  if (missingRoles.length === 0) {
    return {
      fullyExecuted: true,
      missingRoles: [],
      reason: 'all_required_roles_satisfied',
    }
  }
  return {
    fullyExecuted: false,
    missingRoles,
    reason: 'missing_required_roles',
  }
}

/**
 * Idempotent transition helper. `becameFullyExecuted` is true ONLY on the
 * first evaluation that flips an agreement from not-fully-executed to
 * fully-executed for a given version — so callers emit AGREEMENT_FULLY_EXECUTED
 * exactly once (replayed/duplicate evaluations are no-ops).
 */
export function agreementExecutionTransition(
  previous: AgreementExecutionVerdict | null,
  current: AgreementExecutionVerdict,
): { becameFullyExecuted: boolean; verdict: AgreementExecutionVerdict } {
  const becameFullyExecuted =
    current.fullyExecuted && !(previous?.fullyExecuted ?? false)
  return { becameFullyExecuted, verdict: current }
}

// ---------------------------------------------------------------------------
// Required-role policy seam (stop-sign C).
// ---------------------------------------------------------------------------

/**
 * Per-template override of REQUIRED execution roles.
 *
 * CRM-27 (authoritative business decision): PR-PNS requires every actual
 * participant in BUYER, SELLER and SELLER_BROKER. This policy is a decision, not
 * a fixture default. Templates NOT execution-eligible are rejected before the
 * predicate is ever reached (precondition_failure) — they can never emit.
 */
const REQUIRED_EXECUTION_ROLE_OVERRIDES: Record<string, readonly string[]> = {
  'PR-PNS': ['BUYER', 'SELLER', 'SELLER_BROKER'],
}

/**
 * Templates that participate in agreement-execution evaluation for this story.
 * Only PR-PNS (Purchase & Sale) is execution-eligible. An unknown / other
 * template is a precondition_failure — never a vacuous success.
 */
export const EXECUTION_ELIGIBLE_TEMPLATES: ReadonlySet<string> = new Set(['PR-PNS'])

/** Is this template eligible for agreement-execution evaluation? */
export function isExecutionEligibleTemplate(templateId: string): boolean {
  return EXECUTION_ELIGIBLE_TEMPLATES.has(templateId)
}

/**
 * Resolve the REQUIRED execution roles for an issued agreement template.
 *
 * The predicate itself always supports optional roles: an optional role simply
 * must not be listed in the resolved required set.
 */
export function resolveRequiredExecutionRoles(
  templateId: string,
  declaredSignatureRoles: readonly string[],
): readonly string[] {
  return REQUIRED_EXECUTION_ROLE_OVERRIDES[templateId] ?? [...declaredSignatureRoles]
}
