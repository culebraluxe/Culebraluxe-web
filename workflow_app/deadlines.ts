// Deadline model — which canonical business date (if any) feeds a milestone's
// deadline. This is business policy about canonical date sourcing, NOT a
// workflow-state mapping: keys are node ids (the XML state identity) and the
// application remains the canonical source of actual business dates.
//
// Where no canonical date exists, `factSource` is null: the deadline is
// "unresolved" and must not be invented.
//
// CRM-22 — canonical fact sources exist ONLY for milestones justified by
// actual business use:
//   - inspection  -> DealWorkflowFacts.inspectionDeadline  (deal.inspection_deadline —
//                    the P&S inspection-period contingency date)
//   - financing   -> DealWorkflowFacts.financingDeadline   (deal.financing_deadline —
//                    the P&S financing-commitment contingency date)
//   - closing     -> DealWorkflowFacts.closingDate         (deal.closing_date)
// Every other milestone is deliberately unresolved: appraisal / title / tax-
// CRIM / funds / closing-documents deadlines are process-internal obligations
// driven by the closing date, not separate contract dates. Inventing columns
// or timers for them would create a parallel SLA framework (architect brief
// rejects this) — their timers do not exist in the model.

import type { DealWorkflowFacts } from './facts'

export type DeadlineSpec = {
  label: string
  /** Facts field that carries the canonical date, when one exists. */
  factSource: keyof DealWorkflowFacts | null
  note: string
}

export const DEADLINES: Record<string, DeadlineSpec> = {
  inspection: {
    label: 'Inspection deadline',
    // CRM-22 — canonical P&S inspection-period deadline (deal.inspection_deadline).
    factSource: 'inspectionDeadline',
    note: 'Canonical inspection-period contingency date (deal.inspection_deadline).',
  },
  appraisal: {
    label: 'Appraisal deadline',
    factSource: null,
    note: 'No canonical appraisal date exists (appraisal is an obligation within the financing/timing window, not a separate P&S contract date); unresolved — no artificial date is invented.',
  },
  financing: {
    label: 'Financing deadline',
    // CRM-22 — canonical P&S financing-commitment deadline (deal.financing_deadline).
    factSource: 'financingDeadline',
    note: 'Canonical financing-commitment contingency date (deal.financing_deadline).',
  },
  title_work: {
    label: 'Title deadline',
    factSource: null,
    note: 'No canonical title deadline exists (title readiness is a process obligation driven by the closing date, not a separate contract date); unresolved.',
  },
  tax_clearance: {
    label: 'Tax / CRIM clearance deadline',
    factSource: null,
    note: 'No canonical tax/CRIM clearance date exists (a process obligation driven by the closing date, not a separate contract date); unresolved.',
  },
  funds_ready: {
    label: 'Funds readiness',
    factSource: null,
    note: 'No canonical funds readiness date exists (a process obligation driven by the closing date, not a separate contract date); unresolved.',
  },
  closing_documents: {
    label: 'Closing document deadline',
    factSource: null,
    note: 'No canonical closing-document date exists (a process obligation driven by the closing date, not a separate contract date); unresolved.',
  },
  closing: {
    label: 'Closing target',
    factSource: 'closingDate',
    note: 'Deal closing_date is the canonical closing target.',
  },
}

export function deadlineFor(nodeId: string): DeadlineSpec {
  return (
    DEADLINES[nodeId] ?? {
      label: nodeId,
      factSource: null,
      note: 'No deadline policy defined.',
    }
  )
}

/** Returns the deadline label only when a deadline policy is defined. */
export function deadlineLabelFor(nodeId: string): string | null {
  return DEADLINES[nodeId]?.label ?? null
}
