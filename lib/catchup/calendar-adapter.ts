// ---------------------------------------------------------------------------
// CATCH-UP — normalized calendar adapter boundary.
//
// Apple Calendar remains authoritative for Lisa's calendar. CulebraLuxe
// consumes/enriches it. This file defines the normalized application boundary
// (a CalendarEventSource) and the pure event normalizer. The production
// "Mac Calendar edge" adapter (reading the Mac/Apple bridge) is a remaining
// integration task; today the built-in source reads canonical showings, so the
// calendar UI and Catch-Up attention derivation work against REAL data without
// faking production Apple sync.
// ---------------------------------------------------------------------------

export type CatchUpCalendarEvent = {
  id: string
  title: string
  startAt: string
  endAt: string | null
  allDay: boolean
  personId: string | null
  personName: string | null
  propertyName: string | null
  kind: 'showing' | 'meeting' | 'call' | 'other'
  source: string
}

/** Normalized event source — the only seam a real Apple/Mac adapter must satisfy. */
export type CalendarEventSource = {
  listEvents(): Promise<CatchUpCalendarEvent[]>
}

/** Deterministic, human sortable label (PR time). */
export function calendarDayLabel(startAt: string): string {
  const d = new Date(startAt)
  if (Number.isNaN(d.getTime())) return ''
  return d.toLocaleDateString('en-US', {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    timeZone: 'America/Puerto_Rico',
  })
}

export function calendarTimeLabel(startAt: string): string {
  const d = new Date(startAt)
  if (Number.isNaN(d.getTime())) return ''
  return d.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    timeZone: 'America/Puerto_Rico',
  })
}

export function normalizeCalendarEvent(
  input: Omit<CatchUpCalendarEvent, 'startAt' | 'endAt'> & {
    startAt: string
    endAt?: string | null
  },
): CatchUpCalendarEvent {
  return {
    id: input.id,
    title: input.title,
    startAt: toIsoString(input.startAt),
    endAt: input.endAt ? toIsoString(input.endAt) : null,
    allDay: input.allDay,
    personId: input.personId,
    personName: input.personName,
    propertyName: input.propertyName,
    kind: input.kind,
    source: input.source,
  }
}

/** Coerce a DB timestamp (neon returns Date) or ISO string to a plain ISO string. */
function toIsoString(value: string | Date): string {
  return value instanceof Date ? value.toISOString() : value
}

/** Week bucket (Sunday-start, PR tz) for the calendar view. */
export function weekBuckets(anchorMs: number): Array<{
  key: string
  dayLabel: string
  dateLabel: string
}> {
  const anchor = new Date(anchorMs)
  const day = (anchor.getUTCDay() + 6) % 7 // Monday = 0
  const monday = new Date(anchor)
  monday.setUTCDate(anchor.getUTCDate() - day)
  monday.setUTCHours(0, 0, 0, 0)

  const buckets: Array<{ key: string; dayLabel: string; dateLabel: string }> =
    []
  for (let i = 0; i < 7; i += 1) {
    const d = new Date(monday)
    d.setUTCDate(monday.getUTCDate() + i)
    buckets.push({
      key: d.toISOString().slice(0, 10),
      dayLabel: d.toLocaleDateString('en-US', { weekday: 'short' }),
      dateLabel: d.toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
        timeZone: 'UTC',
      }),
    })
  }
  return buckets
}

export function bucketForEvent(
  event: Pick<CatchUpCalendarEvent, 'startAt'>,
  buckets: Array<{ key: string }>,
): string | null {
  const key = new Date(event.startAt).toISOString().slice(0, 10)
  return buckets.some((b) => b.key === key) ? key : null
}
