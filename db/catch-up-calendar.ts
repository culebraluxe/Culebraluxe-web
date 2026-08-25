import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import {
  normalizeCalendarEvent,
  type CatchUpCalendarEvent,
} from '@/lib/catchup/calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — calendar read behind the normalized adapter boundary.
//
// Apple Calendar is authoritative; this is a consuming projection. The
// built-in source today reads canonical showings (real scheduled/completed
// events), so the calendar UI + Catch-Up attention derivation work against
// real data. A production Mac/Apple Calendar edge adapter implements the same
// CalendarEventSource contract and replaces/augments this source.
// ---------------------------------------------------------------------------

type ShowingRow = {
  id: string
  person_id: string | null
  person_name: string | null
  property_name: string | null
  status: string
  scheduled_at: string | null
}

const showingSource = {
  async listEvents(execute: QueryExecutor = sql): Promise<CatchUpCalendarEvent[]> {
    const rows = (await execute`
      select
        s.id,
        s.person_id,
        person.display_name as person_name,
        coalesce(property.name, deal_property.name) as property_name,
        s.status,
        s.scheduled_at
      from showing s
      left join person on person.id = s.person_id
      left join property on property.id = s.property_id
      left join deal d on d.id = s.deal_id
      left join property deal_property on deal_property.id = d.property_id
      where s.status in ('scheduled', 'completed')
        and s.scheduled_at is not null
        and s.scheduled_at >= now() - interval '7 days'
      order by s.scheduled_at asc
      limit 60
    `) as ShowingRow[]

    return rows
      .filter((r): r is ShowingRow & { scheduled_at: string } => !!r.scheduled_at)
      .map((r) =>
        normalizeCalendarEvent({
          id: `showing:${r.id}`,
          title: r.property_name ? `Showing · ${r.property_name}` : 'Showing',
          startAt: r.scheduled_at,
          endAt: null,
          allDay: false,
          personId: r.person_id,
          personName: r.person_name,
          propertyName: r.property_name,
          kind: 'showing',
          source: 'canonical:showing',
        }),
      )
  },
}

/** Real, deterministic calendar projection for the Catch-Up screen. */
export async function getCatchUpCalendarEvents(
  execute: QueryExecutor = sql,
): Promise<CatchUpCalendarEvent[]> {
  return showingSource.listEvents(execute)
}

/**
 * The Apple/Mac Calendar edge task — explicitly deferred, but the contract is
 * stable: implement CalendarEventSource over the Mac/Apple bridge and swap it
 * in here. We never invent a non-existent cloud Apple Calendar API.
 */
export const APPLE_CALENDAR_EDGE = {
  status: 'deferred',
  note: 'Mac/Apple calendar adapter (CalendarEventSource) is the remaining edge task. Today the built-in source reads canonical showings.',
} as const
