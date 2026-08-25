import { readFile } from 'node:fs/promises'

import type { CatchUpCalendarEvent } from './calendar-adapter'

// ---------------------------------------------------------------------------
// MAC-SYNC-CAL-01 — EventKit -> CalendarEventSource normalization.
//
// The Mac bridge (scripts/macbridge/CalendarEventKit.swift) writes a bounded
// JSON snapshot of normalized Apple/iCloud events. This module lowers each
// snapshot entry into the Catch-Up CalendarEventSource contract. No EventKit
// objects leak past the bridge; no person identity is guessed from free-text
// titles (unlinked events render but never drive person-specific attention).
// Idempotency: the stable EKEvent.eventIdentifier becomes the stable event id,
// so re-sync never duplicates and edits are reflected on the next snapshot.
// ---------------------------------------------------------------------------

export type EventKitNormalizedEvent = {
  /** Stable source event (series) id. */
  eventIdentifier: string
  /** Stable per-occurrence id (macOS 14+); absent in the first real snapshot. */
  eventOccurrenceID?: string | null
  sourceAccount: string
  calendarName: string
  title: string
  startAt: string
  endAt: string
  allDay: boolean
  location?: string | null
  notes?: string | null
}

export function eventKitToCatchUpEvent(
  raw: EventKitNormalizedEvent,
): CatchUpCalendarEvent | null {
  if (!raw || typeof raw !== 'object') return null
  if (!raw.eventIdentifier || !raw.title) return null
  const startAt = new Date(raw.startAt)
  const endAt = new Date(raw.endAt)
  if (Number.isNaN(startAt.getTime()) || Number.isNaN(endAt.getTime())) {
    return null
  }
  const startAtIso = startAt.toISOString()
  // Stable per-occurrence identity: prefer EventKit's eventOccurrenceID when the
  // bridge provides it (unique per occurrence, stable across edits). Fall back
  // to (series id + occurrence start) so recurring occurrences stay distinct and
  // replay of the same snapshot is idempotent — never dedup by title/time.
  const id = raw.eventOccurrenceID
    ? `eventkit:${raw.eventOccurrenceID}`
    : `eventkit:${raw.eventIdentifier}:${startAtIso}`
  return {
    id,
    title: raw.title,
    startAt: startAtIso,
    endAt: endAt.toISOString(),
    allDay: Boolean(raw.allDay),
    // Never guess a person from a free-text title — Apple events stay unlinked.
    personId: null,
    personName: null,
    propertyName: null,
    kind: raw.allDay ? 'other' : 'meeting',
    source: 'apple_calendar',
  }
}

export function eventKitSnapshotToCatchUp(
  events: unknown,
): CatchUpCalendarEvent[] {
  if (!Array.isArray(events)) return []
  return events
    .map((e) => eventKitToCatchUpEvent(e as EventKitNormalizedEvent))
    .filter((e): e is CatchUpCalendarEvent => e !== null)
}

/** Path to the Mac bridge's bounded calendar snapshot. Defaults to the local
 *  bridge output (/tmp/culebraluxe-calendar.json) so DEV consumes the real
 *  EventKit snapshot without manual per-file config; MAC_BRIDGE_CALENDAR_JSON
 *  overrides it. /tmp is local runtime data and is never committed. */
export function eventKitSnapshotPath(): string {
  return process.env.MAC_BRIDGE_CALENDAR_JSON || '/tmp/culebraluxe-calendar.json'
}

/**
 * Load the EventKit-derived normalized events from the Mac bridge snapshot.
 * Bounded, idempotent (stable source ids). Returns [] when no snapshot is
 * configured or readable — never a fabricated event.
 */
export async function loadEventKitCalendarEvents(): Promise<
  CatchUpCalendarEvent[]
> {
  const path = eventKitSnapshotPath()
  if (!path) return []
  try {
    const raw = await readFile(path, 'utf8')
    const parsed: unknown = JSON.parse(raw)
    return eventKitSnapshotToCatchUp(parsed)
  } catch {
    return []
  }
}
