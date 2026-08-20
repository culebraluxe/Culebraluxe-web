// Deadline model — which canonical business date (if any) feeds a milestone's
// deadline. This is business policy about canonical date sourcing, NOT a
// workflow-state mapping: keys are node ids (the XML state identity) and the
// application remains the canonical source of actual business dates.
//
// Where no canonical date exists, `factSource` is null: the deadline is
// "unresolved" and must not be invented.

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
    factSource: null,
    note: 'No canonical inspection date exists in the business model today; unresolved.',
  },
  appraisal: {
    label: 'Appraisal deadline',
    factSource: null,
    note: 'No canonical appraisal date exists; unresolved.',
  },
  financing: {
    label: 'Financing deadline',
    factSource: null,
    note: 'No canonical financing deadline exists; unresolved.',
  },
  title_work: {
    label: 'Title deadline',
    factSource: null,
    note: 'No canonical title deadline exists; unresolved.',
  },
  tax_clearance: {
    label: 'Tax / CRIM clearance deadline',
    factSource: null,
    note: 'No canonical tax/CRIM clearance date exists; unresolved.',
  },
  funds_ready: {
    label: 'Funds readiness',
    factSource: null,
    note: 'No canonical funds readiness date exists; unresolved.',
  },
  closing_documents: {
    label: 'Closing document deadline',
    factSource: null,
    note: 'No canonical closing-document date exists; unresolved.',
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
