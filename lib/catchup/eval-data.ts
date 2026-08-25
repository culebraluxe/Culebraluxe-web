import dayjs from 'dayjs'

import type { CatchUpCalendarEvent } from './calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — deterministic evaluation calendar events (CAL-02 A/B).
//
// Temporary, realistic event set so Lisa can compare how both engines render
// the SAME multi-day / all-day / same-day / timed patterns. Anchored to the
// current month so both candidates open on the same month and show the same
// events. This is evaluation data only — remove with the losing candidate.
// ---------------------------------------------------------------------------

export function getEvaluationCalendarEvents(
  base: Date = new Date(),
): CatchUpCalendarEvent[] {
  const monthStart = dayjs(base).startOf('month')
  const at = (day: number, hour: number, minute = 0) =>
    monthStart
      .date(day)
      .hour(hour)
      .minute(minute)
      .second(0)
      .millisecond(0)

  const baseEvent = {
    personId: null,
    personName: null,
    propertyName: null,
  }

  return [
    // 1. one normal timed event
    {
      id: 'eval:meeting-broker',
      title: 'Meeting · Broker check-in',
      startAt: at(3, 10).toISOString(),
      endAt: at(3, 11).toISOString(),
      allDay: false,
      ...baseEvent,
      kind: 'meeting',
      source: 'eval',
    },
    // 2. two events on the same day
    {
      id: 'eval:showing-casa',
      title: 'Showing · Casa Mar',
      startAt: at(5, 9).toISOString(),
      endAt: at(5, 10).toISOString(),
      allDay: false,
      ...baseEvent,
      kind: 'showing',
      source: 'eval',
    },
    {
      id: 'eval:meeting-susan',
      title: 'Meeting · Client Susan',
      startAt: at(5, 14).toISOString(),
      endAt: at(5, 15).toISOString(),
      allDay: false,
      ...baseEvent,
      kind: 'meeting',
      source: 'eval',
    },
    // 3. one all-day event
    {
      id: 'eval:closing',
      title: 'Closing · Villa del Mar',
      startAt: at(8, 0).toISOString(),
      endAt: at(8, 0).toISOString(),
      allDay: true,
      ...baseEvent,
      kind: 'other',
      source: 'eval',
    },
    // 4. one multi-day event spanning several days
    {
      id: 'eval:travel',
      title: 'Travel · Mainland trip',
      startAt: at(12, 0).toISOString(),
      endAt: at(15, 0).toISOString(),
      allDay: true,
      ...baseEvent,
      kind: 'other',
      source: 'eval',
    },
  ]
}
