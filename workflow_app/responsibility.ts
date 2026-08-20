// Application/model-level responsibility for transaction-close-v1 milestones.
// The workflow engine only sees free-string candidateGroups; this policy maps
// those groups to a responsibility class and (for SMEs) the deal_participant
// role_label used to resolve the responsible business participant.

export type ResponsibilityClass =
  | 'brokerage'
  | 'client'
  | 'seller'
  | 'lender'
  | 'inspector'
  | 'appraiser'
  | 'title'
  | 'attorney'
  | 'other'

export type ResponsibilitySpec = {
  owner: ResponsibilityClass
  /** deal_participant.role_label to resolve the responsible SME, if any. */
  smeRoleLabel?: string
  label: string
}

export const RESPONSIBILITY: Record<string, ResponsibilitySpec> = {
  contract_preparation: {
    owner: 'brokerage',
    label: 'Brokerage prepares the purchase contract.',
  },
  contract_executed: {
    owner: 'brokerage',
    label: 'Brokerage obtains a fully executed contract.',
  },
  mark_under_contract: {
    owner: 'brokerage',
    label: 'Brokerage records the deal as under contract.',
  },
  inspection: {
    owner: 'inspector',
    smeRoleLabel: 'inspector',
    label: 'Inspection must be completed by the inspector.',
  },
  inspection_blocker: {
    owner: 'inspector',
    smeRoleLabel: 'inspector',
    label: 'Inspector resolves the inspection blocker.',
  },
  title: {
    owner: 'title',
    smeRoleLabel: 'title',
    label: 'Title work must be completed by the title professional.',
  },
  title_blocker: {
    owner: 'title',
    smeRoleLabel: 'title',
    label: 'Title professional resolves the title blocker.',
  },
  appraisal: {
    owner: 'appraiser',
    smeRoleLabel: 'appraiser',
    label: 'Appraisal is completed by the appraiser when applicable.',
  },
  financing: {
    owner: 'lender',
    smeRoleLabel: 'lender',
    label: 'Financing is completed by the lender when applicable.',
  },
  closing: {
    owner: 'attorney',
    smeRoleLabel: 'attorney',
    label: 'Closing is coordinated by the closing professional.',
  },
  mark_closed: {
    owner: 'brokerage',
    label: 'Brokerage records the deal as closed.',
  },
}

export function responsibilityFor(milestoneId: string): ResponsibilitySpec {
  return (
    RESPONSIBILITY[milestoneId] ?? {
      owner: 'other',
      label: milestoneId,
    }
  )
}
