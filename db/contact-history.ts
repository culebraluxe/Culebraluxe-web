import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type { InteractionChannel, InteractionDirection } from '../lib/crm-types'

// ---------------------------------------------------------------------------
// CLIENTS — contact history for one canonical Person.
//
// Server-side-paginated read over the canonical `interaction` table (the
// application communication timeline: calls, iMessage, SMS, email, meetings /
// showings). Newest-first, SQL ORDER BY occurred_at DESC + LIMIT/OFFSET, so a
// long history never ships to the browser in one payload. Only the current page
// (~20 items) is returned; the component resets to page 1 when the selected
// client changes. No raw L/ODS tables are exposed here.
// ---------------------------------------------------------------------------

export type ContactHistoryItem = {
  id: string
  channel: InteractionChannel
  direction: InteractionDirection | null
  occurredAt: string
  title: string | null
  summary: string | null
}

export type ContactHistoryResult = {
  rows: ContactHistoryItem[]
  total: number
  page: number
  pageSize: number
}

type ContactHistoryRow = {
  id: string
  channel: string
  direction: string | null
  occurred_at: string
  title: string | null
  summary: string | null
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
    from interaction i
    where i.person_id = ${personId}
  `) as { total: number }[]
  const total = Number(countRows[0]?.total ?? 0)

  const rows = (await execute`
    select
      i.id,
      i.channel,
      i.direction,
      to_char(
        i.occurred_at at time zone 'America/Puerto_Rico',
        'Mon FMDD, YYYY HH12:MI AM'
      ) as occurred_at,
      i.title,
      i.summary
    from interaction i
    where i.person_id = ${personId}
    order by i.occurred_at desc, i.id desc
    limit ${pageSize} offset ${offset}
  `) as ContactHistoryRow[]

  return {
    rows: rows.map((r) => ({
      id: r.id,
      channel: r.channel as InteractionChannel,
      direction: (r.direction as InteractionDirection | null) ?? null,
      occurredAt: r.occurred_at,
      title: r.title,
      summary: r.summary,
    })),
    total,
    page,
    pageSize,
  }
}
