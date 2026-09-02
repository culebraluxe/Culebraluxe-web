import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type { ClientRelationshipChannel } from '../lib/portal/types'

// ---------------------------------------------------------------------------
// CLIENTS — source-grain relationship read model (mv_client_relationship_channels).
//
// ONE row per canonical Person x communication source. This is the PRIMARY
// relationship-memory grain for the Client History panel: a small bounded set of
// source nodes, never thousands of messages and never per-burst rows. Counts
// and dates are pre-aggregated in Postgres; the UI never reconstructs them.
// ---------------------------------------------------------------------------

type RelationshipChannelDbRow = {
  person_id: string
  source: string
  channel: string
  first_observed_at: string | null
  last_contact_at: string | null
  last_inbound_at: string | null
  last_outbound_at: string | null
  inbound_count: string | number | bigint
  outbound_count: string | number | bigint
  total_count: string | number | bigint
  last_direction: 'inbound' | 'outbound' | null
  two_way: boolean | null
  last_context: string | null
  last_context_at: string | null
  last_context_type: string | null
  last_context_direction: 'inbound' | 'outbound' | null
}

/** Normalize a Postgres timestamptz (Date), ISO string, or null to ISO or null. */
function toIso(value: string | Date | null | undefined): string | null {
  if (value === null || value === undefined) return null
  const d = value instanceof Date ? value : new Date(value)
  return Number.isNaN(d.getTime()) ? String(value) : d.toISOString()
}

/** Repository boundary normalizes driver-native numerics to JS numbers. */
function toCount(value: string | number | bigint | null | undefined): number {
  if (value == null) return 0
  return typeof value === 'bigint' ? Number(value) : Number(value)
}

export async function getClientRelationshipChannels(
  personId: string,
  execute: QueryExecutor = sql,
): Promise<ClientRelationshipChannel[]> {
  const rows = (await execute`
    select
      person_id, source, channel,
      first_observed_at, last_contact_at, last_inbound_at, last_outbound_at,
      inbound_count, outbound_count, total_count,
      last_direction, two_way,
      last_context, last_context_at, last_context_type, last_context_direction
    from mv_client_relationship_channels
    where person_id = ${personId}
    order by last_contact_at desc nulls last, source asc
  `) as RelationshipChannelDbRow[]

  return rows.map((r) => ({
    personId: r.person_id,
    source: r.source,
    channel: r.channel,
    firstObservedAt: toIso(r.first_observed_at),
    lastContactAt: toIso(r.last_contact_at),
    lastInboundAt: toIso(r.last_inbound_at),
    lastOutboundAt: toIso(r.last_outbound_at),
    inboundCount: toCount(r.inbound_count),
    outboundCount: toCount(r.outbound_count),
    totalCount: toCount(r.total_count),
    lastDirection: r.last_direction,
    twoWay: Boolean(r.two_way),
    lastContext: r.last_context,
    lastContextAt: toIso(r.last_context_at),
    lastContextType: r.last_context_type,
    lastContextDirection: r.last_context_direction,
  }))
}
