import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type { InteractionChannel, InteractionDirection } from '../lib/crm-types'
import { groupIntoBursts } from '../lib/relationship-intel/conversation-bursts'
import type { ConversationBurst } from '../lib/relationship-intel/conversation-bursts'
import { getRelationshipEvidenceForPersons } from './relationship-evidence'
import { summarizeRelationshipEvidence } from '../lib/relationship-intel/relationship-context'
import type { RelationshipChannelProjection } from '../lib/relationship-intel/relationship-context'

// ---------------------------------------------------------------------------
// CLIENTS — contact history for one canonical Person.
//
// Server-side-paginated read over the canonical `interaction` table (the
// application communication timeline) via mv_client_contact_history. Newest
// first, SQL ORDER BY occurred_at DESC + LIMIT/OFFSET, so a long history never
// ships to the browser in one payload. Dense message channels are grouped into
// deterministic conversation bursts (≤30 min gap) so the timeline reads as
// human-sized relationship moments rather than thousands of chat fragments;
// Email / Call / Meeting / Showing / Note pass through as single moments.
//
// In addition to canonical detailed moments, the read model projects ONE bounded
// aggregate-evidence timeline item per communication source that has observed
// activity in integration_relationship_evidence but no corresponding detailed
// canonical interactions. This keeps communication counted in "Observed
// Communications" visible (e.g. Gmail aggregate email) without fabricating
// individual events or duplicating a source already represented by detail.
// ---------------------------------------------------------------------------

export type ContactHistoryMoment = ConversationBurst & {
  channel: InteractionChannel
  preview: string | null
  kind: 'detail'
}

/**
 * A bounded aggregate timeline item projected from relationship evidence for a
 * communication source that has observed activity but no detailed canonical
 * interactions. Clearly typed `kind: 'aggregate_evidence'` so the UI can
 * distinguish it from a detailed canonical event. Never masquerades as detail.
 */
export type AggregateEvidenceHistoryItem = {
  kind: 'aggregate_evidence'
  id: string
  channel: InteractionChannel
  source: string
  firstObservedAt: string | null
  lastObservedAt: string | null
  inboundCount: number
  outboundCount: number
  totalCount: number
  isTwoWay: boolean
}

export type ContactHistoryRow = ContactHistoryMoment | AggregateEvidenceHistoryItem

export type ContactHistoryResult = {
  rows: ContactHistoryRow[]
  total: number
  page: number
  pageSize: number
}

type ContactHistoryDbRow = {
  interaction_id: string
  channel: string
  direction: string | null
  occurred_at: string | Date
  title: string | null
  summary: string | null
}

/** Normalize a Postgres timestamptz (Date) or ISO string to an ISO string. */
function toIso(value: string | Date): string {
  const d = value instanceof Date ? value : new Date(value)
  return Number.isNaN(d.getTime()) ? String(value) : d.toISOString()
}

/**
 * Pure projection: build aggregate-evidence timeline items from a source's
 * channel projections, excluding sources already represented by detailed
 * canonical interactions (`coveredSources` = distinct interaction.source_system)
 * and sources without communication counts. Identity-only sources (e.g. Apple
 * Contacts) never appear here because they have no channel projection/counts.
 */
export function buildAggregateEvidenceItems(
  channels: RelationshipChannelProjection[],
  coveredSources: Set<string>,
): AggregateEvidenceHistoryItem[] {
  return channels
    .filter((c) => c.observedCommunicationCount > 0 && !coveredSources.has(c.source))
    .map((c) => ({
      kind: 'aggregate_evidence' as const,
      id: `aggregate:${c.source}`,
      channel: c.channel as InteractionChannel,
      source: c.source,
      firstObservedAt: c.firstObservedAt,
      lastObservedAt: c.lastObservedAt,
      inboundCount: c.inboundCount,
      outboundCount: c.outboundCount,
      totalCount: c.observedCommunicationCount,
      isTwoWay: c.twoWay,
    }))
}

/** Effective date for timeline sorting: burst end for detail, lastObserved for evidence. */
function effectiveDate(row: ContactHistoryRow): string {
  if (row.kind === 'aggregate_evidence') return row.lastObservedAt ?? ''
  return row.endedAt ?? row.startedAt ?? ''
}


export async function getClientContactHistory(
  personId: string,
  opts: { page?: number; pageSize?: number },
  execute: QueryExecutor = sql,
): Promise<ContactHistoryResult> {
  const page = Math.max(1, opts.page ?? 1)
  const pageSize = Math.max(1, Math.min(50, opts.pageSize ?? 20))
  const offset = (page - 1) * pageSize

  const countRows = (await execute`
    select count(*)::int as total
    from mv_client_contact_history mv
    where mv.person_id = ${personId}
  `) as { total: number }[]
  const total = Number(countRows[0]?.total ?? 0)

  const rows = (await execute`
    select
      mv.interaction_id,
      mv.channel,
      mv.direction,
      mv.occurred_at,
      mv.title,
      mv.summary
    from mv_client_contact_history mv
    where mv.person_id = ${personId}
    order by mv.occurred_at desc, mv.interaction_id desc
    limit ${pageSize} offset ${offset}
  `) as ContactHistoryDbRow[]

  const moments = groupIntoBursts(
    rows.map((r) => ({
      id: r.interaction_id,
      channel: r.channel,
      direction: (r.direction as InteractionDirection | null) ?? null,
      occurredAt: toIso(r.occurred_at),
      preview: r.title ?? r.summary ?? null,
    })),
  ).map((m) => ({ ...m, channel: m.channel as InteractionChannel, kind: 'detail' as const }))

  // --- Aggregate evidence-only communication items -------------------------
  // Surface sources counted in Observed Communications that have no detailed
  // canonical interactions (e.g. Gmail aggregate email) as ONE bounded,
  // clearly-typed evidence-only item per source. A source already represented
  // by detailed interactions (same source_system) is NOT duplicated, and
  // identity-only sources (e.g. Apple Contacts) never produce a communication
  // item because they have no channel projection/counts.
  const evidenceByPerson = await getRelationshipEvidenceForPersons([personId], execute)
  const evidence = evidenceByPerson[personId] ?? []
  const summary = summarizeRelationshipEvidence(evidence)

  const coveredRows = (await execute`
    select distinct source_system as source
    from interaction
    where person_id = ${personId} and source_system is not null
  `) as { source: string }[]
  const coveredSources = new Set(coveredRows.map((r) => r.source))

  const aggregates = buildAggregateEvidenceItems(summary.channels, coveredSources)

  return {
    rows: [...moments, ...aggregates].sort((a, b) =>
      effectiveDate(b).localeCompare(effectiveDate(a)),
    ),
    total,
    page,
    pageSize,
  }
}
