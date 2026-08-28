// ---------------------------------------------------------------------------
// REL-INTEL — conversation-burst projection for message channels.
//
// Dense chat history must read as human-sized relationship moments, not as
// thousands of visually equivalent timeline cards. This pure projection groups
// per-client + per-channel message events into deterministic conversation
// bursts WITHOUT destroying the underlying per-event interaction history.
//
// Rule (named constant): messages for the same client + message channel that
// are separated by <= 30 minutes belong to one burst; a gap > 30 minutes
// starts a new burst.
// ---------------------------------------------------------------------------

/** Max gap between messages that still counts as one conversation. */
export const BURST_THRESHOLD_MS = 30 * 60 * 1000 // 30 minutes

export type BurstEvent = {
  id: string
  channel: string
  direction: 'inbound' | 'outbound' | null
  /** ISO timestamp (sortable). */
  occurredAt: string
  /** Bounded one-line memory cue, if available. */
  preview: string | null
}

export type BurstDirection = 'inbound' | 'outbound' | 'two-way' | null

export type ConversationBurst = {
  id: string
  channel: string
  direction: BurstDirection
  startedAt: string
  endedAt: string
  count: number
  inboundCount: number
  outboundCount: number
  twoWay: boolean
  /** Deterministic: the latest non-empty message preview in the burst. */
  preview: string | null
}

/** Group a channel's events (chronological) into bursts by the gap rule. */
function burstOneChannel(events: BurstEvent[]): ConversationBurst[] {
  const sorted = [...events].sort((a, b) =>
    a.occurredAt.localeCompare(b.occurredAt),
  )
  const bursts: ConversationBurst[] = []
  let current: BurstEvent[] = []
  let lastTime = 0
  for (const e of sorted) {
    const t = new Date(e.occurredAt).getTime()
    if (current.length > 0 && t - lastTime > BURST_THRESHOLD_MS) {
      bursts.push(finalizeBurst(current))
      current = []
    }
    current.push(e)
    lastTime = t
  }
  if (current.length > 0) bursts.push(finalizeBurst(current))
  return bursts
}

function finalizeBurst(events: BurstEvent[]): ConversationBurst {
  const first = events[0]
  const last = events[events.length - 1]
  const inboundCount = events.filter((e) => e.direction === 'inbound').length
  const outboundCount = events.filter((e) => e.direction === 'outbound').length
  const twoWay = inboundCount > 0 && outboundCount > 0
  const direction: BurstDirection = twoWay
    ? 'two-way'
    : inboundCount > 0
      ? 'inbound'
      : outboundCount > 0
        ? 'outbound'
        : null
  return {
    id: first.id,
    channel: first.channel,
    direction,
    startedAt: first.occurredAt,
    endedAt: last.occurredAt,
    count: events.length,
    inboundCount,
    outboundCount,
    twoWay,
    preview: latestNonEmptyPreview(events),
  }
}

/** Deterministic memory-trigger preview: latest non-empty message in the burst. */
function latestNonEmptyPreview(events: BurstEvent[]): string | null {
  for (let i = events.length - 1; i >= 0; i--) {
    if (events[i].preview) return events[i].preview
  }
  return null
}

/**
 * Group a set of message events into conversation bursts, newest-first.
 * Only message channels are burst-grouped; everything else passes through as a
 * single-event burst so the timeline can still show Email / Call / Note etc.
 */
export function groupIntoBursts(events: BurstEvent[]): ConversationBurst[] {
  const MESSAGE_CHANNELS = new Set(['imessage', 'sms', 'whatsapp'])

  const byChannel = new Map<string, BurstEvent[]>()
  const singles: ConversationBurst[] = []
  for (const e of events) {
    if (!MESSAGE_CHANNELS.has(e.channel)) {
      singles.push(finalizeBurst([e]))
      continue
    }
    const arr = byChannel.get(e.channel) ?? []
    arr.push(e)
    byChannel.set(e.channel, arr)
  }

  const bursts: ConversationBurst[] = [...singles]
  for (const channelEvents of byChannel.values()) {
    bursts.push(...burstOneChannel(channelEvents))
  }

  // Newest-first for the timeline.
  return bursts.sort((a, b) => b.endedAt.localeCompare(a.endedAt))
}
