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
 *  executed (all required execution slots satisfied, or authorized
 *  manual/external execution). Fired exactly once per version. */
export const AGREEMENT_FULLY_EXECUTED: DomainEventType = 'AGREEMENT_FULLY_EXECUTED'

/**
 * An immutable issued participant / signature slot. Snapshot AT ISSUANCE into the
 * immutable issued-document lineage (transaction_document.source_snapshot.
 * issuedParticipants), so the required participant set is what it was when that
 * specific version was issued — NOT the mutable draft state that may change
 * later. `slotId` is a stable, provider-neutral identity (e.g. "BUYER:1").
 */
export type IssuedExecutionSlot = {
  slotId: string
  role: string
  personId: string | null
  name: string
  email: string | null
  required: boolean
  order: number
}

export type AgreementExecutionEvidence = {
  /** Immutable issued agreement/document version this evidence is about. */
  documentVersion: string
  /** Required execution slots (resolved via the policy seam). */
  requiredSlots: readonly IssuedExecutionSlot[]
  /** Issued slot ids with satisfied COMPLETED signature-request evidence. */
  satisfiedSlotIds: readonly string[]
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
  /** The unsatisfied required slot ids (participant cardinality). */
  missingSlotIds: readonly string[]
  reason: AgreementExecutionReason
}

/**
 * Pure predicate: compute whether the evidence proves full execution of this
 * specific document version.
 *
 *   partial required signatures      -> not fully executed (missingSlots listed)
 *   final required evidence arrives  -> fully executed
 *   authorized manual/external       -> fully executed (satisfies the predicate)
 *
 * PARTICIPANT CARDINALITY (CRM-27): a role is complete only when EVERY issued
 * required slot in that role has execution evidence. Satisfaction is keyed by
 * the ISSUED SLOT ID, so duplicate evidence for one participant can never
 * satisfy another participant in the same role.
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
      missingSlotIds: [],
      reason: 'manual_execution',
    }
  }
  const required = evidence.requiredSlots.filter((slot) => slot.required)
  const satisfied = new Set(evidence.satisfiedSlotIds)
  const missing = required.filter((slot) => !satisfied.has(slot.slotId))
  if (missing.length === 0) {
    return {
      fullyExecuted: true,
      missingRoles: [],
      missingSlotIds: [],
      reason: 'all_required_roles_satisfied',
    }
  }
  return {
    fullyExecuted: false,
    missingRoles: [...new Set(missing.map((slot) => slot.role))],
    missingSlotIds: missing.map((slot) => slot.slotId),
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
 * PR-PNS requires every actual participant in BUYER, SELLER and SELLER_BROKER.
 * LISTING-01 requires every SELLER plus SELLER_BROKER. The broker slot may be
 * satisfied locally by Lisa's issuance-bound signature, so the provider
 * envelope contains only the remaining external Seller slots.
 */
const REQUIRED_EXECUTION_ROLE_OVERRIDES: Record<string, readonly string[]> = {
  'PR-PNS': ['BUYER', 'SELLER', 'SELLER_BROKER'],
  'LISTING-01': ['SELLER', 'SELLER_BROKER'],
}

/** Templates that use immutable participant slots + one ordered envelope. */
export const EXECUTION_ELIGIBLE_TEMPLATES: ReadonlySet<string> = new Set([
  'PR-PNS',
  'LISTING-01',
])

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

/**
 * PR-PNS authoritative required-role set (CRM-27 participant-cardinality policy).
 * Retained for slot construction compatibility; template-specific requiredness
 * is applied by resolveRequiredSlots at issuance/send/evaluation boundaries.
 */
export const PR_PNS_REQUIRED_ROLES: ReadonlySet<string> = new Set([
  'BUYER',
  'SELLER',
  'SELLER_BROKER',
])

/**
 * Build stable, provider-neutral issued slots from the resolved participant
 * collection AT ISSUANCE. slotId is deterministic (`ROLE:sequence`), so multiple
 * people in one role (e.g. two Buyers) become distinct slots ("BUYER:1",
 * "BUYER:2") that must each carry execution evidence.
 */
export function buildIssuedExecutionSlots(
  people: ReadonlyArray<{
    role: string
    personId: string | null
    name: string
    email: string | null
  }>,
): IssuedExecutionSlot[] {
  const counts = new Map<string, number>()
  return people.map((person, index) => {
    const seq = (counts.get(person.role) ?? 0) + 1
    counts.set(person.role, seq)
    return {
      slotId: `${person.role}:${seq}`,
      role: person.role,
      personId: person.personId ?? null,
      name: person.name,
      email: person.email ?? null,
      required: PR_PNS_REQUIRED_ROLES.has(person.role),
      order: index,
    }
  })
}

/** Resolve required issued slots according to the eligible template's policy. */
export function resolveRequiredSlots(
  templateId: string,
  issuedSlots: readonly IssuedExecutionSlot[],
): IssuedExecutionSlot[] {
  if (!isExecutionEligibleTemplate(templateId)) return []
  const requiredRoles = new Set(REQUIRED_EXECUTION_ROLE_OVERRIDES[templateId] ?? [])
  return issuedSlots
    .filter((slot) => requiredRoles.has(slot.role))
    .map((slot) => ({ ...slot, required: true }))
}
