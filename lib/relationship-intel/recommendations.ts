// ---------------------------------------------------------------------------
// CORE-DAILY-07 — deterministic, explainable relationship recommendations (PURE).
//
// A recommendation answers: who needs attention? why? based on what evidence?
// No opaque AI scoring, no manufactured urgency, no fabrication from bulk email.
// Rules are deterministic and each yields an explanation code + short human
// reason + evidence pointers. Dismissed keys are excluded. Partial Gmail
// coverage never becomes stronger truth than the intended rule.
// ---------------------------------------------------------------------------

export type RecommendationCode =
  | 'overdue_relationship_commitment'
  | 'due_soon_relationship_commitment'
  | 'unanswered_inbound'
  | 'two_way_without_next_step'
  | 'quiet_past_client'

export type Recommendation = {
  personId: string
  code: RecommendationCode
  reason: string
  explanationCode: string
  evidencePointers: string[]
  dueAt: string | null
  followUpId: string | null
}

export type RecommendationInput = {
  personId: string
  /** open / due (or due-snoozed) follow-ups for this person */
  followUps: Array<{
    id: string
    dueAt: string | null
    title: string | null
  }>
  /** meaningful (non-bulk/service) relationship evidence summary */
  evidence?: {
    lastMeaningfulContactAt: string | null
    lastInboundAt: string | null
    lastOutboundAt: string | null
    twoWay: boolean
    hasEvidence: boolean
    coverageLimited: boolean
  }
  /** dismissed keys: `${personId}:${code}` */
  dismissed: Set<string>
  nowIso: string
  dueSoonWindowMs: number
  quietAfterMs: number
}

const REASONS: Record<RecommendationCode, string> = {
  overdue_relationship_commitment: 'An open follow-up is overdue',
  due_soon_relationship_commitment: 'An open follow-up is due soon',
  unanswered_inbound: 'A recent inbound has not been answered',
  two_way_without_next_step: 'Two-way relationship without a next step',
  quiet_past_client: 'This client has gone quiet',
}

/**
 * Evaluate deterministic recommendations for one person, in stable order.
 * A dismissed key is never re-emitted.
 */
export function evaluateRecommendations(input: RecommendationInput): Recommendation[] {
  const now = Date.parse(input.nowIso)
  const out: Recommendation[] = []
  const dismissed = (code: RecommendationCode) =>
    input.dismissed.has(`${input.personId}:${code}`)

  const hasOpenFollowUp = input.followUps.length > 0
  const openSorted = [...input.followUps].sort((a, b) =>
    (a.dueAt ?? '9999').localeCompare(b.dueAt ?? '9999'),
  )

  // 1) Overdue relationship commitment.
  const overdue = openSorted.find((f) => f.dueAt !== null && Date.parse(f.dueAt!) < now)
  if (overdue) {
    if (!dismissed('overdue_relationship_commitment')) {
      out.push({
        personId: input.personId, code: 'overdue_relationship_commitment',
        reason: REASONS.overdue_relationship_commitment,
        explanationCode: 'overdue_due_at_lt_now',
        evidencePointers: [`task:${overdue.id}`],
        dueAt: overdue.dueAt, followUpId: overdue.id,
      })
    }
    // An overdue item is handled by the overdue rule only — dismissing it must
    // not silently fall through to due-soon and defeat the suppression.
    return out
  }

  // 2) Due-soon commitment.
  const dueSoon = openSorted.find(
    (f) => f.dueAt !== null && Date.parse(f.dueAt!) < now + input.dueSoonWindowMs,
  )
  if (dueSoon && !dismissed('due_soon_relationship_commitment')) {
    out.push({
      personId: input.personId, code: 'due_soon_relationship_commitment',
      reason: REASONS.due_soon_relationship_commitment,
      explanationCode: 'due_soon_within_window',
      evidencePointers: [`task:${dueSoon.id}`],
      dueAt: dueSoon.dueAt, followUpId: dueSoon.id,
    })
    return out
  }

  const ev = input.evidence
  if (!ev) return out

  // 3) Unanswered inbound (meaningful, non-bulk evidence only).
  if (
    !hasOpenFollowUp &&
    ev.lastInboundAt &&
    (!ev.lastOutboundAt || ev.lastInboundAt > ev.lastOutboundAt) &&
    !dismissed('unanswered_inbound')
  ) {
    out.push({
      personId: input.personId, code: 'unanswered_inbound',
      reason: REASONS.unanswered_inbound,
      explanationCode: 'inbound_after_last_outbound',
      evidencePointers: [`inbound_at:${ev.lastInboundAt}`],
      dueAt: ev.lastInboundAt, followUpId: null,
    })
    return out
  }

  // 4) Two-way relationship without a next step.
  if (
    !hasOpenFollowUp && ev.twoWay &&
    !dismissed('two_way_without_next_step')
  ) {
    out.push({
      personId: input.personId, code: 'two_way_without_next_step',
      reason: REASONS.two_way_without_next_step,
      explanationCode: 'two_way_no_open_followup',
      evidencePointers: ['relationship_evidence:two_way'],
      dueAt: null, followUpId: null,
    })
    return out
  }

  // 5) Quiet past client.
  if (
    !hasOpenFollowUp && ev.hasEvidence &&
    ev.lastMeaningfulContactAt &&
    Date.parse(ev.lastMeaningfulContactAt) < now - input.quietAfterMs &&
    !dismissed('quiet_past_client')
  ) {
    out.push({
      personId: input.personId, code: 'quiet_past_client',
      reason: REASONS.quiet_past_client,
      explanationCode: 'meaningful_contact_before_quiet_threshold',
      evidencePointers: [`last_meaningful_contact:${ev.lastMeaningfulContactAt}`],
      dueAt: null, followUpId: null,
    })
    return out
  }

  return out
}

