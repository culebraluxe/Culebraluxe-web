// ---------------------------------------------------------------------------
// CATCH-UP — deterministic attention rules (PURE, no DB/React).
//
// Catch-Up answers one question: "WHO needs Lisa today, and why?" Every queue
// row must explain itself in ordinary language. These rules are deterministic
// and explainable — no opaque scores. A person only appears if a rule produces
// a human-readable reason.
// ---------------------------------------------------------------------------

export type CatchUpReasonCode =
  | 'new_lead'
  | 'needs_response'
  | 'calendar_attention'
  | 'active_deal_attention'
  | 'stale_tickle'

export type CatchUpAttention = {
  reasonCode: CatchUpReasonCode
  /** One human-readable sentence: WHY this person is here. */
  reasonLabel: string
  /** Lower = more urgent. */
  priority: number
}

/** Derived per-person facts assembled by the Catch-Up read model. */
export type CatchUpPersonFacts = {
  id: string
  displayName: string
  role: string
  status: string
  email: string | null
  phone: string | null
  createdAt: string
  lastMeaningfulContactAt: string | null
  lastInboundAt: string | null
  lastOutboundAt: string | null
  activeDealStage: string | null
  activeDealProperty: string | null
  nextEventAt: string | null
  nextEventLabel: string | null
}

// Tuning knobs (seconds). Deterministic; tuned with the product, not invented
// per row.
export const RULE_WINDOWS = {
  newLeadDays: 5,
  needsResponseDays: 2,
  calendarLookaheadDays: 7,
  dealContactDays: 5,
  staleDays: 10,
} as const

const DAY = 24 * 60 * 60 * 1000

function daysSince(iso: string | null, now: number): number | null {
  if (!iso) return null
  const ms = new Date(iso).getTime()
  if (Number.isNaN(ms)) return null
  return Math.max(0, Math.floor((now - ms) / DAY))
}

function withinDays(iso: string | null, days: number, now: number): boolean {
  const d = daysSince(iso, now)
  return d !== null && d < days
}

function daysAgoLabel(days: number): string {
  if (days === 0) return 'today'
  if (days === 1) return 'yesterday'
  return `${days} days`
}

function dealRoleLabel(stage: string): string {
  switch (stage) {
    case 'under_contract':
      return 'Closing'
    case 'offer':
      return 'Offer'
    case 'showing':
      return 'Showing'
    case 'qualified':
      return 'Qualified'
    default:
      return 'Deal'
  }
}

function roleLabel(role: string): string {
  if (role === 'both') return 'Buyer & Seller'
  return role.charAt(0).toUpperCase() + role.slice(1)
}

/**
 * Derive the single most-urgent attention reason for a person, or null if the
 * person has no reason to be in Catch-Up today.
 */
export function deriveAttention(
  facts: CatchUpPersonFacts,
  now: number = Date.now(),
): CatchUpAttention | null {
  const candidates: CatchUpAttention[] = []

  // 1. NEW LEAD — newly created website lead with no meaningful response yet.
  if (
    withinDays(facts.createdAt, RULE_WINDOWS.newLeadDays, now) &&
    !facts.lastOutboundAt
  ) {
    candidates.push({
      reasonCode: 'new_lead',
      reasonLabel: `New ${facts.status === 'new' ? 'lead' : roleLabel(facts.role)} · ${daysAgoLabel(
        daysSince(facts.createdAt, now) ?? 0,
      )}, no response yet`,
      priority: 0,
    })
  }

  // 2. NEEDS RESPONSE — recent inbound with no later outbound.
  if (
    withinDays(facts.lastInboundAt, RULE_WINDOWS.needsResponseDays, now) &&
    (!facts.lastOutboundAt ||
      (facts.lastInboundAt &&
        facts.lastOutboundAt &&
        facts.lastOutboundAt < facts.lastInboundAt))
  ) {
    candidates.push({
      reasonCode: 'needs_response',
      reasonLabel: `Inbound message ${daysAgoLabel(
        daysSince(facts.lastInboundAt, now) ?? 0,
      )} · no later response`,
      priority: 1,
    })
  }

  // 3. CALENDAR-DRIVEN — upcoming relevant event.
  if (withinDays(facts.nextEventAt, RULE_WINDOWS.calendarLookaheadDays, now)) {
    candidates.push({
      reasonCode: 'calendar_attention',
      reasonLabel:
        facts.nextEventLabel ??
        'Upcoming scheduled event · may need attention',
      priority: 1,
    })
  }

  // 4. ACTIVE DEAL — participant in an active deal, quiet recently.
  if (facts.activeDealStage && !facts.activeDealProperty) {
    // (property not required to qualify)
  }
  if (
    facts.activeDealStage &&
    daysSince(facts.lastMeaningfulContactAt, now) !== null &&
    (daysSince(facts.lastMeaningfulContactAt, now) ?? 0) >=
      RULE_WINDOWS.dealContactDays
  ) {
    const quietDays = daysSince(facts.lastMeaningfulContactAt, now) ?? 0
    candidates.push({
      reasonCode: 'active_deal_attention',
      reasonLabel: `${dealRoleLabel(facts.activeDealStage)} · no contact in ${daysAgoLabel(
        quietDays,
      )}`,
      priority: 2,
    })
  }

  // 5. STALE TICKLE — eligible relationship, quiet long enough.
  if (
    facts.lastMeaningfulContactAt !== null &&
    daysSince(facts.lastMeaningfulContactAt, now) !== null &&
    (daysSince(facts.lastMeaningfulContactAt, now) ?? 0) >=
      RULE_WINDOWS.staleDays
  ) {
    const quietDays = daysSince(facts.lastMeaningfulContactAt, now) ?? 0
    candidates.push({
      reasonCode: 'stale_tickle',
      reasonLabel: `No meaningful contact in ${daysAgoLabel(quietDays)}`,
      priority: 3,
    })
  }

  if (candidates.length === 0) return null
  candidates.sort((a, b) => a.priority - b.priority)
  return candidates[0]
}

/** Ordered severity list, for the "one excellent queue" sort. */
export const REASON_ORDER: CatchUpReasonCode[] = [
  'new_lead',
  'needs_response',
  'calendar_attention',
  'active_deal_attention',
  'stale_tickle',
]
