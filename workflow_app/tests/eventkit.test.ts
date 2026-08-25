import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import {
  eventKitToCatchUpEvent,
  eventKitSnapshotToCatchUp,
  type EventKitNormalizedEvent,
} from '../../lib/catchup/eventkit'

// ---------------------------------------------------------------------------
// MAC-SYNC-CAL-01 — EventKit bridge -> CalendarEventSource normalization proofs.
// ---------------------------------------------------------------------------

const base: EventKitNormalizedEvent = {
  eventIdentifier: 'EK-0001-ABCD',
  sourceAccount: 'apple-calendar:local',
  calendarName: 'Personal',
  title: 'Broker check-in',
  startAt: '2026-08-27T16:00:00.000Z',
  endAt: '2026-08-27T17:00:00.000Z',
  allDay: false,
  location: 'Culebra',
  notes: 'Confirm documents',
}

test('eventkit: normalizes an EventKit event into CalendarEventSource', () => {
  const e = eventKitToCatchUpEvent(base)
  assert.ok(e)
  assert.equal(e!.id, 'eventkit:EK-0001-ABCD:2026-08-27T16:00:00.000Z')
  assert.equal(e!.title, 'Broker check-in')
  assert.equal(e!.startAt, '2026-08-27T16:00:00.000Z')
  assert.equal(e!.endAt, '2026-08-27T17:00:00.000Z')
  assert.equal(e!.allDay, false)
  assert.equal(e!.kind, 'meeting')
  assert.equal(e!.source, 'apple_calendar')
  assert.equal(e!.personId, null, 'Apple events stay unlinked')
})

test('eventkit: idempotent — the stable eventIdentifier yields a stable id', () => {
  const a = eventKitToCatchUpEvent(base)
  const b = eventKitToCatchUpEvent({ ...base })
  assert.equal(a!.id, b!.id, 'same source event -> same id (no duplicate)')
  const ids = eventKitSnapshotToCatchUp([base, base, base]).map((e) => e.id)
  assert.equal(new Set(ids).size, 1, 'repeated snapshot never duplicates')
})

test('eventkit: eventOccurrenceID gives a stable id across edits (edit-proof)', () => {
  const before = eventKitToCatchUpEvent({ ...base, eventOccurrenceID: 'OCC-1' })!
  const after = eventKitToCatchUpEvent({
    ...base,
    eventOccurrenceID: 'OCC-1',
    title: 'Broker check-in (moved)',
    startAt: '2026-08-28T10:00:00.000Z',
  })!
  assert.equal(before.id, after.id, 'per-occurrence id stable across edit')
  assert.notEqual(before.title, after.title, 'title edit reflected')
  assert.notEqual(before.startAt, after.startAt, 'time edit reflected')
})

test('eventkit: fallback id reflects a time edit as a new occurrence, no stale duplicate', () => {
  const before = eventKitToCatchUpEvent(base)!
  const edited = eventKitToCatchUpEvent({
    ...base,
    startAt: '2026-08-28T10:00:00.000Z',
  })!
  assert.notEqual(before.id, edited.id, 'time change -> new occurrence identity')
  // replay of the OLD snapshot still yields the OLD id (no duplicate drift)
  assert.equal(eventKitToCatchUpEvent(base)!.id, before.id)
})

test('eventkit: recurring occurrences of one series stay distinct', () => {
  const a = eventKitToCatchUpEvent(base)!
  const b = eventKitToCatchUpEvent({
    ...base,
    startAt: '2026-09-03T16:00:00.000Z',
  })!
  assert.notEqual(a.id, b.id, 'different occurrences are distinct events')
  const ids = new Set(
    eventKitSnapshotToCatchUp([base, { ...base, startAt: '2026-09-03T16:00:00.000Z' }]).map(
      (e) => e.id,
    ),
  )
  assert.equal(ids.size, 2, 'both occurrences render distinctly')
})

test('eventkit: does not guess a person from free-text titles', () => {
  const named = eventKitToCatchUpEvent({
    ...base,
    title: 'Lunch with Maria Perez',
  })!
  assert.equal(named.personId, null)
  assert.equal(named.personName, null)
})

test('eventkit: invalid entries are filtered, not fabricated', () => {
  const events = eventKitSnapshotToCatchUp([
    base,
    { ...base, eventIdentifier: '' },
    { ...base, title: '' },
    { ...base, startAt: 'not-a-date' },
    { ...base, endAt: 'not-a-date' },
    'not-an-object',
    null,
  ])
  assert.equal(events.length, 1, 'only the valid entry survives')
})

test('eventkit: non-array snapshot yields nothing', () => {
  assert.equal(eventKitSnapshotToCatchUp(null).length, 0)
  assert.equal(eventKitSnapshotToCatchUp({}).length, 0)
})

test('eventkit: all-day events map to kind "other"', () => {
  const e = eventKitToCatchUpEvent({ ...base, allDay: true })
  assert.equal(e!.allDay, true)
  assert.equal(e!.kind, 'other')
})

test('eventkit: calendar seam merges EventKit events with showings (source guard)', () => {
  const src = readFileSync(
    new URL('../../db/catch-up-calendar.ts', import.meta.url),
    'utf8',
  )
  assert.ok(/loadEventKitCalendarEvents/.test(src), 'consumes EventKit events')
  assert.ok(/showingSource/.test(src), 'keeps canonical showings')
})
