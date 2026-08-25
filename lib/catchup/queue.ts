import {
  deriveAttention,
  type CatchUpAttention,
  type CatchUpPersonFacts,
} from './rules'
import type { CatchUpEligibleRow } from '@/db/catch-up'

// ---------------------------------------------------------------------------
// CATCH-UP — assemble the derived queue from the eligible read-model rows.
// Applies the deterministic, explainable rules and keeps only people with a
// reason, in urgency order. Pure and unit-testable.
// ---------------------------------------------------------------------------

export type CatchUpQueueItem = CatchUpEligibleRow &
  CatchUpAttention & {
    facts: CatchUpPersonFacts
  }

export function toFacts(row: CatchUpEligibleRow): CatchUpPersonFacts {
  return {
    id: row.personId,
    displayName: row.displayName,
    role: row.role,
    status: row.status,
    email: row.email,
    phone: row.phone,
    createdAt: row.createdAt,
    lastMeaningfulContactAt: row.lastMeaningfulContactAt,
    lastInboundAt: row.lastInboundAt,
    lastOutboundAt: row.lastOutboundAt,
    activeDealStage: row.activeDealStage,
    activeDealProperty: row.activeDealProperty,
    nextEventAt: row.nextEventAt,
    nextEventLabel: row.nextEventLabel,
  }
}

/** Keep only eligible people who have a reason to be in Catch-Up, urgency first. */
export function buildCatchUpQueue(
  rows: CatchUpEligibleRow[],
): CatchUpQueueItem[] {
  return rows
    .map((row) => {
      const facts = toFacts(row)
      const attention = deriveAttention(facts)
      if (!attention) return null
      return { ...row, ...attention, facts }
    })
    .filter((item): item is CatchUpQueueItem => item !== null)
    .sort(
      (a, b) =>
        a.priority - b.priority ||
        a.displayName.localeCompare(b.displayName),
    )
}
