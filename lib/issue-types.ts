// ---------------------------------------------------------------------------
// OPS-11A — Operational Issue vocabulary + deterministic runbook config.
//
// The `issue` table is the ONLY durable surface. This module is the code/config
// source for:
//   - the closed set of issue types we generate
//   - the Support/OPPS responsibility mapping (SUPPORT_EXCEPTION vs
//     OPERATIONS_EXCEPTION) — a deterministic type -> responsibility rule, not
//     a separate table
//   - per-type runbook guidance (code/config driven, no runbook CMS)
// No AI scoring, no alert/escalation/subscriber machinery.
// ---------------------------------------------------------------------------

export type IssueType =
  | 'MISSING_EXECUTED_PS'
  | 'APPRAISAL_OVERDUE'
  | 'CLOSING_DATE_AT_RISK'
  | 'OVERDUE_DEAL_TASK'

export type IssueSeverity = 'RED' | 'YELLOW' | 'INFO'

export type IssueState = 'OPEN' | 'RESOLVED'

/** Deterministic responsibility bucket — the issue table is shared; this rule
 *  filters a surface. OPPS shows OPERATIONS_EXCEPTION, SUPPORT shows
 *  SUPPORT_EXCEPTION. Never duplicated business logic across screens. */
export type IssueResponsibility = 'OPERATIONS_EXCEPTION' | 'SUPPORT_EXCEPTION'

export type RunbookStep = {
  title: string
  body: string
}

export type IssueRunbook = {
  label: string
  /** One-line "why this matters" — shown as the runbook summary. */
  summary: string
  steps: RunbookStep[]
}

export const ISSUE_TYPES: readonly IssueType[] = [
  'MISSING_EXECUTED_PS',
  'APPRAISAL_OVERDUE',
  'CLOSING_DATE_AT_RISK',
  'OVERDUE_DEAL_TASK',
]

/** Closed set of types surfaced on the OPPS operations dashboard. */
export const OPERATIONS_TYPES: readonly IssueType[] = [
  'MISSING_EXECUTED_PS',
  'APPRAISAL_OVERDUE',
  'CLOSING_DATE_AT_RISK',
  'OVERDUE_DEAL_TASK',
]

export const SUPPORT_TYPES: readonly IssueType[] = []

export function responsibilityForType(type: string): IssueResponsibility {
  if ((OPERATIONS_TYPES as readonly string[]).includes(type)) {
    return 'OPERATIONS_EXCEPTION'
  }
  if ((SUPPORT_TYPES as readonly string[]).includes(type)) {
    return 'SUPPORT_EXCEPTION'
  }
  // Unknown type is treated as an operations exception so it still surfaces to
  // a human rather than vanishing; the runbook degrades gracefully.
  return 'OPERATIONS_EXCEPTION'
}

export function typesForResponsibility(
  responsibility: IssueResponsibility,
): readonly IssueType[] {
  return responsibility === 'SUPPORT_EXCEPTION'
    ? SUPPORT_TYPES
    : OPERATIONS_TYPES
}

export function issueTypeLabel(type: string): string {
  return RUNBOOK[type as IssueType]?.label ?? type
}

/** Deterministic operational runbook per issue type. Code/config driven. */
export const RUNBOOK: Record<IssueType, IssueRunbook> = {
  MISSING_EXECUTED_PS: {
    label: 'Purchase Agreement Not Executed',
    summary:
      'The deal is under contract but has no fully-executed purchase agreement on file.',
    steps: [
      {
        title: 'Confirm the signed document',
        body: 'Verify a transaction_document of type agreement exists in state signed (or the agreement_execution marker) for this deal. If it does, the canonical record is out of date — fix the record rather than clearing the issue.',
      },
      {
        title: 'If not yet signed',
        body: 'Reach the client/parties to complete and sign the purchase agreement. Use the deal workspace to open the agreement and issue it for signature.',
      },
      {
        title: 'After execution',
        body: 'Mark the agreement signed in the canonical document state. The next reconcile will resolve this issue automatically.',
      },
    ],
  },
  APPRAISAL_OVERDUE: {
    label: 'Appraisal Overdue',
    summary:
      'An appraisal is required and closing is near, but no signed appraisal is on file.',
    steps: [
      {
        title: 'Check lender / appraiser status',
        body: 'Confirm who ordered the appraisal and the expected completion date with the lender or appraiser.',
      },
      {
        title: 'Confirm expected completion',
        body: 'If the appraisal is scheduled but not complete, note the expected completion date. Escalate if it threatens the closing date.',
      },
      {
        title: 'Update canonical record',
        body: 'If the appraisal is complete, upload the signed appraisal as a transaction_document (document_type appraisal, state signed) so reconcile resolves the issue.',
      },
    ],
  },
  CLOSING_DATE_AT_RISK: {
    label: 'Closing Date at Risk',
    summary:
      'The closing date is imminent for an under-contract deal and has not yet closed.',
    steps: [
      {
        title: 'Confirm closing readiness',
        body: 'Open the deal and confirm all closing prerequisites are satisfied (title, funds, lender clearance, signed documents).',
      },
      {
        title: 'Close or reschedule',
        body: 'If the deal closes, move the deal stage to closed. If the date changed via amendment, update the canonical closing date rather than clearing the issue.',
      },
      {
        title: 'Escalate if blocked',
        body: 'If a blocker exists (funds, title, lender), escalate to the responsible party before the date passes.',
      },
    ],
  },
  OVERDUE_DEAL_TASK: {
    label: 'Overdue Deal Task',
    summary:
      'An open task on a deal is past its due date and requires attention.',
    steps: [
      {
        title: 'Review the task',
        body: 'Open the deal and inspect the overdue task to understand what is outstanding and who owns it.',
      },
      {
        title: 'Complete or reschedule',
        body: 'If done, mark the task completed in the canonical record. If the date moved, update the task due date rather than clearing the issue.',
      },
      {
        title: 'Follow up',
        body: 'If the task is blocked, follow up with the assignee before it impacts the transaction timeline.',
      },
    ],
  },
}

// ---------------------------------------------------------------------------
// Severity presentation (elegant, not noisy) — shared by the queue UI.
// ---------------------------------------------------------------------------

export type SeverityStyle = {
  /** solid dot for the dense queue rail */
  dot: string
  /** soft badge for the detail header / list pill */
  badge: string
  /** row label class */
  label: string
}

export const SEVERITY_STYLE: Record<IssueSeverity, SeverityStyle> = {
  RED: {
    dot: 'bg-[var(--portal-archive)]',
    badge:
      'bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]',
    label: 'text-[var(--portal-archive)]',
  },
  YELLOW: {
    dot: 'bg-[var(--portal-gold)]',
    badge: 'bg-[var(--portal-gold-pale)] text-[var(--portal-gold-muted)]',
    label: 'text-[var(--portal-gold-muted)]',
  },
  INFO: {
    dot: 'bg-[var(--portal-navy-soft)]',
    badge: 'bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]',
    label: 'text-[var(--portal-navy-soft)]',
  },
}

