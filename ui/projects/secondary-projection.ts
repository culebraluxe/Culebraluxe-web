import type { WbsItem } from '@/services/wbs'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'

export type ProjectCalendarItem = {
  id: string
  title: string
  startAt: string
  status: WbsItem['status']
  category: WbsItem['category']
  owner: string | null
}

/** Project-scoped calendar projection. No due date means no invented event. */
export function mapProjectCalendarItems(items: readonly WbsItem[]): ProjectCalendarItem[] {
  return items
    .filter((item): item is WbsItem & { dueAt: string } => Boolean(item.dueAt && !Number.isNaN(new Date(item.dueAt).getTime())))
    .sort((a, b) => a.dueAt.localeCompare(b.dueAt) || a.id.localeCompare(b.id))
    .map((item) => ({ id: item.id, title: item.title, startAt: item.dueAt, status: item.status, category: item.category, owner: item.owner }))
}

/**
 * The same project dated work, expressed as calendar events for the Projects
 * month view (the shared FullCalendar widget consumes `CatchUpCalendarEvent`).
 *
 * Two deliberate choices:
 *
 * - `kind` is 'other' for every event. The adapter's kinds are
 *   showing/meeting/call/other, and a WBS due date is a work deadline — not a
 *   meeting. Picking a kind to win a colour would put a false semantic into the
 *   event model. When WBS deadlines earn their own visual class, add it to the
 *   adapter; do not overload 'showing'.
 * - `allDay` is true because `mapProjectCalendarItems` carries a due DATE, not a
 *   clock time. Inventing an hour would place work at a time nobody chose.
 *
 * Pure: no fetching, no React, no DB.
 */
export function mapProjectCalendarToEvents(
  items: readonly ProjectCalendarItem[],
): CatchUpCalendarEvent[] {
  return items.map((item) => ({
    id: item.id,
    title: item.title,
    startAt: item.startAt,
    endAt: null,
    allDay: true,
    personId: null,
    personName: null,
    propertyName: null,
    kind: 'other',
    source: 'wbs',
  }))
}
