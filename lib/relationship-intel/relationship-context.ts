// ---------------------------------------------------------------------------
// REL-INTEL — pure relationship-context summarization (Catch-Up + Clients).
//
// Distinguishes last-observed communication from last meaningful contact and
// never lets bulk/service rows refresh "meaningful" freshness. Partial coverage
// is surfaced honestly (never a fabricated complete history). Pure function, no
// database, so the read-model distinctions are unit-testable.
// ---------------------------------------------------------------------------

export type RelationshipEvidenceForContext = {
  source: string
  inboundCount?: number | null
  outboundCount?: number | null
  lastObservedAt?: string | null
  lastInboundAt?: string | null
  lastOutboundAt?: string | null
  isTwoWay?: boolean | null
  isAutomatedOrBulk?: boolean | null
  isOrganizationOrService?: boolean | null
  hasEmail?: boolean | null
  hasPhone?: boolean | null
  coverageNote?: string | null
}

export type RelationshipContextSummary = {
  hasEvidence: boolean
  sources: string[]
  inboundCount: number
  outboundCount: number
  observedCommunicationCount: number
  lastObservedAt: string | null
  lastMeaningfulContactAt: string | null
  lastInboundAt: string | null
  lastOutboundAt: string | null
  twoWay: boolean
  hasEmail: boolean
  hasPhone: boolean
  coverageLimited: boolean
  reason: string | null
  /**
   * Source-specific communication projection (per evidence SOURCE, e.g.
   * apple_messages -> imessage, gmail_contacts -> email). Lets the UI show
   * truthful per-channel "last observed / inbound / outbound" relationship
   * memory. Only sources that carry communication evidence appear here.
   */
  channels: RelationshipChannelProjection[]
}

/** One source's truthful relationship-memory projection. */
export type RelationshipChannelProjection = {
  source: string
  channel: string
  observedCommunicationCount: number
  inboundCount: number
  outboundCount: number
  lastObservedAt: string | null
  lastInboundAt: string | null
  lastOutboundAt: string | null
  twoWay: boolean
  coverageLimited: boolean
}

/** Source-system identifier -> canonical interaction channel. */
const CHANNEL_BY_SOURCE: Record<string, string> = {
  apple_messages: 'imessage',
  gmail_contacts: 'email',
}

function latest(values: Array<string | null | undefined>): string | null {
  const present = values.filter((v): v is string => Boolean(v))
  return present.length > 0 ? [...present].sort().pop() ?? null : null
}

/**
 * Pure summary over neutral evidence rows for one canonical person.
 * Bulk/service rows are excluded from meaningful-contact computations, so they
 * can never make a relationship appear freshly contacted.
 */
export function summarizeRelationshipEvidence(
  evidence: RelationshipEvidenceForContext[],
): RelationshipContextSummary {
  if (evidence.length === 0) {
    return {
      hasEvidence: false, sources: [], lastObservedAt: null, lastMeaningfulContactAt: null,
      lastInboundAt: null, lastOutboundAt: null, twoWay: false, hasEmail: false,
      hasPhone: false, coverageLimited: false, reason: null, inboundCount: 0,
      outboundCount: 0, observedCommunicationCount: 0, channels: [],
    }
  }

  const meaningful = evidence.filter((e) => !e.isAutomatedOrBulk && !e.isOrganizationOrService)
  const lastObservedAt = latest(evidence.map((e) => e.lastObservedAt))
  const lastMeaningfulContactAt = latest(
    meaningful.map((e) => e.lastObservedAt ?? e.lastOutboundAt ?? e.lastInboundAt),
  )
  const lastInboundAt = latest(meaningful.map((e) => e.lastInboundAt))
  const lastOutboundAt = latest(meaningful.map((e) => e.lastOutboundAt))

  // Source-specific relationship-memory projection (truthful per channel).
  const bySource = new Map<string, RelationshipEvidenceForContext[]>()
  for (const e of evidence) {
    if (!CHANNEL_BY_SOURCE[e.source]) continue
    const arr = bySource.get(e.source) ?? []
    arr.push(e)
    bySource.set(e.source, arr)
  }
  const channels: RelationshipChannelProjection[] = []
  for (const [source, rows] of bySource) {
    const inboundCount = rows.reduce((total, e) => total + (e.inboundCount ?? 0), 0)
    const outboundCount = rows.reduce((total, e) => total + (e.outboundCount ?? 0), 0)
    channels.push({
      source,
      channel: CHANNEL_BY_SOURCE[source],
      observedCommunicationCount: inboundCount + outboundCount,
      inboundCount,
      outboundCount,
      lastObservedAt: latest(rows.map((e) => e.lastObservedAt)),
      lastInboundAt: latest(rows.map((e) => e.lastInboundAt)),
      lastOutboundAt: latest(rows.map((e) => e.lastOutboundAt)),
      twoWay: rows.some((e) => e.isTwoWay),
      coverageLimited: rows.some((e) => e.coverageNote),
    })
  }
  channels.sort((a, b) => (b.lastObservedAt ?? '').localeCompare(a.lastObservedAt ?? ''))

  const sources = Array.from(new Set(evidence.map((e) => e.source)))
  const twoWay = evidence.some((e) => e.isTwoWay)
  const hasEmail = evidence.some((e) => e.hasEmail)
  const hasPhone = evidence.some((e) => e.hasPhone)
  const coverageLimited = evidence.some((e) => e.coverageNote)
  const inboundCount = evidence.reduce((total, e) => total + (e.inboundCount ?? 0), 0)
  const outboundCount = evidence.reduce((total, e) => total + (e.outboundCount ?? 0), 0)

  let reason: string | null = null
  if (lastMeaningfulContactAt) {
    const direction =
      lastInboundAt && lastOutboundAt ? 'both directions' : 'one direction'
    reason = `Last meaningful contact ${new Date(lastMeaningfulContactAt).toLocaleDateString('en-US', { month: 'short', year: 'numeric' })} (${direction}${twoWay ? ', two-way' : ''})`
  } else if (evidence.some((e) => e.isOrganizationOrService)) {
    reason = 'Service/organization evidence only — no meaningful personal contact'
  } else if (evidence.some((e) => e.isAutomatedOrBulk)) {
    reason = 'Bulk/automated evidence only — never treated as fresh contact'
  }

  return {
    hasEvidence: true, sources, lastObservedAt, lastMeaningfulContactAt,
    lastInboundAt, lastOutboundAt, twoWay, hasEmail, hasPhone,
    coverageLimited, reason, inboundCount, outboundCount,
    observedCommunicationCount: inboundCount + outboundCount,
    channels,
  }
}
