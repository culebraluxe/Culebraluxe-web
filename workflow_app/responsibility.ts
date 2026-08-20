// ---------------------------------------------------------------------------
// Responsibility / SME contract (Story 117).
//
// Workflow XML declares *abstract business-role hints* on nodes, e.g.
// responsibility="inspector" or responsibility="notario". The engine treats
// them as free-string metadata (and mirrors them into candidateGroups on task
// nodes); it NEVER resolves them to application identity.
//
// This module is where workflow_app resolves a hint to the CulebraLuxe
// operational owner class and, where relevant, the deal_participant role_label
// used to find the actual responsible SME. The resolution target is one of:
//   - owning CulebraLuxe agent (brokerage)
//   - deal_participant (buyer / seller / lender / ...)
//   - person
//   - external SME (inspector, appraiser, notario, title_company, other_sme)
//
// Important distinction preserved from the existing model:
//   deal owner / accountable agent  !=  current task responsible SME
//
// No second SME taxonomy is introduced: the XML hint vocabulary is the same
// conceptual responsibility model the workflow already uses, expressed on the
// node instead of in a node-id -> owner lookup table.
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
    label: 'Buyer',
  },
  seller: {
    owner: 'seller',
    smeRoleLabel: 'seller',
    label: 'Seller / owner',
  },
  lender: {
    owner: 'lender',
    smeRoleLabel: 'lender',
    label: 'Lender',
  },
  inspector: {
    owner: 'inspector',
    smeRoleLabel: 'inspector',
    label: 'Inspector (external SME)',
  },
  appraiser: {
    owner: 'appraiser',
    smeRoleLabel: 'appraiser',
    label: 'Appraiser (external SME)',
  },
  notario: {
    owner: 'notario',
    smeRoleLabel: 'notario',
    label: 'Notary / closing professional',
  },
  title_company: {
    owner: 'title',
    smeRoleLabel: 'title',
    label: 'Title company / title professional',
  },
  other_sme: {
    owner: 'other',
    smeRoleLabel: 'other',
    label: 'External specialist',
  },
}

export const RESPONSIBILITY_HINTS_SET: ReadonlySet<string> = new Set(
  Object.keys(RESPONSIBILITY_HINTS),
)

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
