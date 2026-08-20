// Deadline model for transaction-close-v1.
//
// The application remains the canonical source of actual business dates. The
// engine's `timer` node capability is available for deadline escalation, but
// this first slice materializes milestones as canonical CulebraLuxe tasks
// whose `due_at` is sourced from the facts projection below.
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
  title: {
    label: 'Title deadline',
    factSource: null,
    note: 'No canonical title deadline exists; unresolved.',
  },
  closing: {
    label: 'Closing target',
    factSource: 'closingDate',
    note: 'Deal closing_date is the canonical closing target.',
  },
}

export function deadlineFor(milestoneId: string): DeadlineSpec {
  return (
    DEADLINES[milestoneId] ?? {
      label: milestoneId,
      factSource: null,
      note: 'No deadline policy defined.',
    }
  )
}
