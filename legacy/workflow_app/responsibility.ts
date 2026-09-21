// ---------------------------------------------------------------------------
// Responsibility / SME contract (Story 117, reconciled CRM-13).
//
// Workflow XML declares *abstract business-role hints* on nodes, e.g.
// responsibility="inspector" or responsibility="notario". The engine treats
// them as free-string metadata (and mirrors them into candidateGroups on task
// nodes); it NEVER resolves them to application identity.
//
// This module is where workflow_app resolves a hint to the CulebraLuxe
// operational owner class and, where relevant, the deal_participant used to
// find the actual responsible SME. The resolution target is one of:
//   - owning CulebraLuxe agent (brokerage)
//   - deal_participant (buyer / seller / lender / ...)
//   - person
//   - external SME (inspector, appraiser, notario, title_company, other_sme)
//
// Important distinction preserved from the existing model:
//   deal owner / accountable agent  !=  current task responsible SME
//
// CRM-13 reconciliation: deal_participant is the canonical participant model.
// A responsibility hint resolves to a participant via role_label exactly as
// follows:
//   - buyer  -> structural role='client'  (no role_label)
//   - seller -> structural role='seller'  (no role_label)
//   - lender / inspector / appraiser / notario / title_company
//            -> long tail: role='other' + role_label (the SME long tail is
//               free-form role_label under role='other'; no schema migration
//               per new SME role)
//   - brokerage / other_sme -> no participant target (agent / unspecified)
//
// The workflow responsibility hint remains a SEPARATE XML-node concept that
// merely resolves to a participant — the two taxonomies are not merged.
// ---------------------------------------------------------------------------

export type ResponsibilityHint =
  | 'brokerage'
  | 'buyer'
  | 'seller'
  | 'lender'
  | 'inspector'
  | 'appraiser'
  | 'notario'
  | 'title_company'
  | 'other_sme'

export type ResponsibilityClass =
  | 'brokerage'
  | 'client'
  | 'seller'
  | 'lender'
  | 'inspector'
  | 'appraiser'
  | 'title'
  | 'attorney'
  | 'notario'
  | 'other'

export type ResponsibilitySpec = {
  /** High-level owner class for display/aggregation. */
  owner: ResponsibilityClass
  /** deal_participant.role_label to resolve the responsible SME, if any. */
  smeRoleLabel?: string
  /**
   * deal_participant role category the hint resolves to (CRM-13):
   *   - 'client' / 'seller' — structural role, no role_label needed;
   *   - 'other'            — SME long tail: role='other' + role_label.
   * Absent for hints with no participant target (brokerage, other_sme).
   */
  participantRole?: 'client' | 'seller' | 'other'
  /** Human-readable description of the responsibility. */
  label: string
}

export const RESPONSIBILITY_HINTS: Record<ResponsibilityHint, ResponsibilitySpec> = {
  brokerage: {
    owner: 'brokerage',
    label: 'Owning CulebraLuxe agent',
  },
  buyer: {
    owner: 'client',
    smeRoleLabel: 'buyer',
    participantRole: 'client',
    label: 'Buyer',
  },
  seller: {
    owner: 'seller',
    smeRoleLabel: 'seller',
    participantRole: 'seller',
    label: 'Seller / owner',
  },
  lender: {
    owner: 'lender',
    smeRoleLabel: 'lender',
    participantRole: 'other',
    label: 'Lender',
  },
  inspector: {
    owner: 'inspector',
    smeRoleLabel: 'inspector',
    participantRole: 'other',
    label: 'Inspector (external SME)',
  },
  appraiser: {
    owner: 'appraiser',
    smeRoleLabel: 'appraiser',
    participantRole: 'other',
    label: 'Appraiser (external SME)',
  },
  notario: {
    owner: 'notario',
    smeRoleLabel: 'notario',
    participantRole: 'other',
    label: 'Notary / closing professional',
  },
  title_company: {
    owner: 'title',
    smeRoleLabel: 'title',
    participantRole: 'other',
    label: 'Title company / title professional',
  },
  other_sme: {
    owner: 'other',
    label: 'External specialist',
  },
}

export const RESPONSIBILITY_HINTS_SET: ReadonlySet<string> = new Set(
  Object.keys(RESPONSIBILITY_HINTS),
)

/**
 * Canonical SME long-tail role_labels (deal_participant.role='other' +
 * role_label) the workflow can ask for — derived from the hint table so the
 * vocabulary cannot drift. New SME roles are curated labels, never schema
 * migrations.
 */
export const LONG_TAIL_ROLE_LABELS: ReadonlyArray<string> = Object.values(
  RESPONSIBILITY_HINTS,
)
  .filter((spec) => spec.participantRole === 'other' && spec.smeRoleLabel)
  .map((spec) => spec.smeRoleLabel as string)

/**
 * Resolve an XML responsibility hint (or a raw candidate group) to its
 * operational spec. Unknown hints fall back to a neutral 'other' spec rather
 * than throwing, so the read path degrades gracefully.
 */
export function resolveResponsibility(hint: string | undefined | null): ResponsibilitySpec {
  if (hint && RESPONSIBILITY_HINTS_SET.has(hint)) {
    return RESPONSIBILITY_HINTS[hint as ResponsibilityHint]
  }
  return {
    owner: 'other',
    label: hint && hint.length > 0 ? hint : 'Unassigned',
  }
}

/**
 * The deal_participant lookup a resolved hint maps to, if any (CRM-13):
 *   - structural: role='client' or role='seller' (no role_label);
 *   - long_tail: role='other' + roleLabel (the SME long tail).
 * Returns null for hints with no participant target (brokerage, other_sme,
 * unknown hints).
 */
export type ParticipantResolution =
  | { kind: 'structural'; role: 'client' | 'seller' }
  | { kind: 'long_tail'; role: 'other'; roleLabel: string }

export function resolveParticipantTarget(
  spec: ResponsibilitySpec,
): ParticipantResolution | null {
  if (spec.participantRole === 'client' || spec.participantRole === 'seller') {
    return { kind: 'structural', role: spec.participantRole }
  }
  if (spec.participantRole === 'other' && spec.smeRoleLabel) {
    return { kind: 'long_tail', role: 'other', roleLabel: spec.smeRoleLabel }
  }
  return null
}
