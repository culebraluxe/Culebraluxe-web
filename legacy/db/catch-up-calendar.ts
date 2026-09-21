import { sql } from '@/legacy/db/client'
import type { QueryExecutor } from '@/legacy/db/query-executor'
import {
  normalizeCalendarEvent,
  type CatchUpCalendarEvent,
} from '@/lib/catchup/calendar-adapter'
import { loadEventKitCalendarEvents } from '@/lib/catchup/eventkit'

// ---------------------------------------------------------------------------
// CATCH-UP — calendar read behind the normalized adapter boundary.
//
// Apple Calendar is authoritative; CulebraLuxe consumes it. Canonical showings
// come straight from the application model. Apple/iCloud events arrive through
// the Mac EventKit edge, land replay-safely in l_calendar, and are read from
// there in production. Local DEV may also read the bounded /tmp EventKit
// snapshot; ids are deduped at this boundary so local + landed copies never
// render twice.
// ---------------------------------------------------------------------------

type ShowingRow = {
  id: string
  person_id: string | null
  person_name: string | null
  property_name: string | null
  status: string
  scheduled_at: string | null
}

type LandingCalendarRow = {
  id: string
  source_account: string | null
  source_message_id: string
  title: string | null
  starts_at: string | null
  ends_at: string | null
  all_day: boolean | null
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

export function landingCalendarRowsToCatchUp(
  rows: readonly LandingCalendarRow[],
): CatchUpCalendarEvent[] {
  return rows
    .filter((row): row is LandingCalendarRow & { starts_at: string } => Boolean(row.starts_at))
    .map((row) =>
      normalizeCalendarEvent({
        id: row.source_message_id || `landing-calendar:${row.id}`,
        title: row.title?.trim() || 'Calendar event',
        startAt: row.starts_at,
        endAt: row.ends_at,
        allDay: Boolean(row.all_day),
        personId: null,
        personName: null,
        propertyName: null,
        kind: row.all_day ? 'other' : 'meeting',
        source: 'apple_calendar',
      }),
    )
}

const appleLandingSource = {
  async listEvents(execute: QueryExecutor = sql): Promise<CatchUpCalendarEvent[]> {
    const rows = (await execute`
      select
        id,
        source_account,
        source_message_id,
        title,
        starts_at,
        ends_at,
        all_day
      from l_calendar
      where starts_at is not null
        and starts_at >= now() - interval '7 days'
        and starts_at < now() + interval '60 days'
      order by starts_at asc
      limit 500
    `) as LandingCalendarRow[]
    return landingCalendarRowsToCatchUp(rows)
  },
}

function dedupeAndSort(events: readonly CatchUpCalendarEvent[]): CatchUpCalendarEvent[] {
  const byId = new Map<string, CatchUpCalendarEvent>()
  for (const event of events) byId.set(event.id, event)
  return Array.from(byId.values()).sort((a, b) => a.startAt.localeCompare(b.startAt) || a.id.localeCompare(b.id))
}

/** Real calendar projection for Catch-Up: canonical showings + durable Apple
 * Calendar landing rows. Local EventKit snapshot data is also accepted for DEV
 * proof/use; the stable EventKit id makes that merge replay-safe. */
export async function getCatchUpCalendarEvents(
  execute: QueryExecutor = sql,
): Promise<CatchUpCalendarEvent[]> {
  const [showings, landedApple, localEventKit] = await Promise.all([
    showingSource.listEvents(execute),
    appleLandingSource.listEvents(execute),
    loadEventKitCalendarEvents(),
  ])
  return dedupeAndSort([...showings, ...landedApple, ...localEventKit])
}

export const APPLE_CALENDAR_EDGE = {
  status: 'wired',
  note: 'EventKit snapshot -> l_calendar landing -> Catch-Up CalendarEventSource. Apple Calendar remains authoritative/read-only.',
} as const
