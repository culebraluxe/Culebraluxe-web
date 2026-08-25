import dayjs from 'dayjs'

import type { CatchUpCalendarEvent } from './calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — normalized -> engine event mappers (PURE).
//
// One normalized CatchUpCalendarEvent is mapped to each engine's event shape so
// both calendar candidates render the SAME data with the SAME timestamps. No
// engine-specific data model is created.
// ---------------------------------------------------------------------------

function resolveSpan(event: CatchUpCalendarEvent) {
  const start = event.startAt
  const end =
    event.endAt ?? dayjs(event.startAt).add(1, 'hour').toISOString()
  return { start, end }
}

/** Map a normalized event to the @ilamy/calendar event shape. */
export function toIlamyCalendarEvent(event: CatchUpCalendarEvent) {
  const { start, end } = resolveSpan(event)
  const gold = event.kind === 'showing'
  return {
    id: event.id,
    title: event.title,
    start,
    end,
    allDay: event.allDay,
    color: gold ? '#c6a15b' : '#3f6ea5',
    backgroundColor: gold ? 'rgba(198,161,91,0.18)' : 'rgba(63,110,165,0.14)',
  }
}

/** Map a normalized event to the FullCalendar event shape. */
export function toFullCalendarEvent(event: CatchUpCalendarEvent) {
  const { start, end } = resolveSpan(event)
  const gold = event.kind === 'showing'
  const base = gold ? '#c6a15b' : '#3f6ea5'
  return {
    id: event.id,
    title: event.title,
    start,
    end,
    allDay: event.allDay,
    backgroundColor: base,
    borderColor: base,
    textColor: '#ffffff',
  }
}
