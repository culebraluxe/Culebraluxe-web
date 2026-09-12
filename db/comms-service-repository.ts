import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type { RelationshipEvidenceForContext } from '../lib/relationship-intel/relationship-context'
import type {
  CommsMomentPage,
  CommsMomentRecord,
  CommsRepository,
  CommsSourceRecord,
} from '../services/comms'

// ---------------------------------------------------------------------------
// COMMS — persistence boundary. Warehouse read models only.
//
//   mv_client_relationship_channels     one row per Person x source, with counts,
//                                       dates and the newest bounded context
//   mv_client_directory                 the pre-formatted last-contact label
//   integration_relationship_evidence   neutral evidence for the header summary
//   interaction                         the canonical comms event timeline
//
// No ODS (l_) table is read here, and no channel mapping happens here either: the
// raw source is passed through so the service owns that rule.
//
// Repository boundary normalization: driver-native values (Date, BigInt, the
// numeric strings Postgres returns for bigint) become ISO strings and JS numbers
// before they leave this module.
// ---------------------------------------------------------------------------

/** Postgres timestamptz (Date) or an ISO string -> ISO string or null. */
function iso(value: unknown): string | null {
  if (value === null || value === undefined) return null
  const date = value instanceof Date ? value : new Date(String(value))
  return Number.isNaN(date.getTime()) ? null : date.toISOString()
}

/** bigint / numeric string / number -> safe JS number. */
function num(value: unknown): number {
  if (value === null || value === undefined) return 0
  const parsed = typeof value === 'number' ? value : Number(value)
  return Number.isFinite(parsed) ? parsed : 0
}

function direction(value: unknown): 'inbound' | 'outbound' | null {
  return value === 'inbound' || value === 'outbound' ? value : null
}

function text(value: unknown): string | null {
  if (value === null || value === undefined) return null
  const trimmed = String(value).trim()
  return trimmed.length > 0 ? trimmed : null
}

export class SqlCommsRepository implements CommsRepository {
  constructor(private readonly execute: QueryExecutor = sql) {}

  async sources(personId: string): Promise<CommsSourceRecord[]> {
    const rows = (await this.execute`
      select source, first_observed_at, last_contact_at, last_inbound_at, last_outbound_at,
             inbound_count, outbound_count, total_count, last_direction, two_way,
             last_context, last_context_at, last_context_type, last_context_direction
        from mv_client_relationship_channels
       where person_id = ${personId}
       order by last_contact_at desc nulls last, source asc
    `) as unknown as Array<Record<string, unknown>>

    return rows.map((row) => ({
      source: String(row.source),
      firstObservedAt: iso(row.first_observed_at),
      lastContactAt: iso(row.last_contact_at),
      lastInboundAt: iso(row.last_inbound_at),
      lastOutboundAt: iso(row.last_outbound_at),
      inboundCount: num(row.inbound_count),
      outboundCount: num(row.outbound_count),
      totalCount: num(row.total_count),
      twoWay: Boolean(row.two_way),
      lastDirection: direction(row.last_direction),
      lastContext: text(row.last_context),
      lastContextAt: iso(row.last_context_at),
      lastContextType: text(row.last_context_type),
      lastContextDirection: direction(row.last_context_direction),
    }))
  }

  async evidence(personId: string): Promise<RelationshipEvidenceForContext[]> {
    const rows = (await this.execute`
      select source, first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
             inbound_count, outbound_count, is_two_way,
             is_automated_or_bulk, is_organization_or_service, has_email, has_phone
        from integration_relationship_evidence
       where canonical_person_id = ${personId}
       order by source asc
    `) as unknown as Array<Record<string, unknown>>

    const flag = (value: unknown): boolean | null => (value === null || value === undefined ? null : Boolean(value))

    return rows.map((row) => ({
      source: String(row.source),
      firstObservedAt: iso(row.first_observed_at),
      lastObservedAt: iso(row.last_observed_at),
      lastInboundAt: iso(row.last_inbound_at),
      lastOutboundAt: iso(row.last_outbound_at),
      inboundCount: num(row.inbound_count),
      outboundCount: num(row.outbound_count),
      isTwoWay: Boolean(row.is_two_way),
      isAutomatedOrBulk: flag(row.is_automated_or_bulk),
      isOrganizationOrService: flag(row.is_organization_or_service),
      hasEmail: flag(row.has_email),
      hasPhone: flag(row.has_phone),
    }))
  }

  async lastContact(personId: string): Promise<{ at: string | null; label: string | null }> {
    const rows = (await this.execute`
      select last_contact_at, last_contact_label
        from mv_client_directory
       where person_id = ${personId}
    `) as unknown as Array<Record<string, unknown>>
    const row = rows[0]
    return { at: row ? iso(row.last_contact_at) : null, label: row ? text(row.last_contact_label) : null }
  }

  async moments(personId: string, limit: number, offset: number): Promise<CommsMomentPage> {
    const rows = (await this.execute`
      select id, channel, source_system, direction, occurred_at, title, summary
        from interaction
       where person_id = ${personId}
       order by occurred_at desc, id desc
       limit ${limit} offset ${offset}
    `) as unknown as Array<Record<string, unknown>>

    const counted = (await this.execute`
      select count(*)::int as n from interaction where person_id = ${personId}
    `) as unknown as Array<{ n: unknown }>

    const moments: CommsMomentRecord[] = rows.map((row) => ({
      id: String(row.id),
      sourceSystem: text(row.source_system),
      direction: direction(row.direction),
      occurredAt: iso(row.occurred_at) ?? '',
      title: text(row.title),
      summary: text(row.summary),
    }))

    return { moments, total: num(counted[0]?.n) }
  }
}
