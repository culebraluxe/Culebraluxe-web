import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type { InteractionChannel, InteractionDirection } from '../lib/crm-types'
import { groupIntoBursts } from '../lib/relationship-intel/conversation-bursts'
import type { ConversationBurst } from '../lib/relationship-intel/conversation-bursts'

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
// ---------------------------------------------------------------------------

export type ContactHistoryMoment = ConversationBurst & {
  channel: InteractionChannel
  preview: string | null
}

export type ContactHistoryResult = {
  rows: ContactHistoryMoment[]
  total: number
  page: number
  pageSize: number
}

type ContactHistoryRow = {
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
  `) as ContactHistoryRow[]

  const moments = groupIntoBursts(
    rows.map((r) => ({
      id: r.interaction_id,
      channel: r.channel,
      direction: (r.direction as InteractionDirection | null) ?? null,
      occurredAt: toIso(r.occurred_at),
      preview: r.title ?? r.summary ?? null,
    })),
  ).map((m) => ({ ...m, channel: m.channel as InteractionChannel }))

  return {
    rows: moments,
    total,
    page,
    pageSize,
  }
}
