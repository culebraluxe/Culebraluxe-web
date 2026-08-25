import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import {
  toIlamyCalendarEvent,
  toFullCalendarEvent,
} from '../../lib/catchup/calendar-mappers'
import { getEvaluationCalendarEvents } from '../../lib/catchup/eval-data'
import type { CatchUpCalendarEvent } from '../../lib/catchup/calendar-adapter'

// ---------------------------------------------------------------------------
// CAL-02 — Catch-Up calendar A/B (ilamy vs FullCalendar) focused proofs:
// shared mappers, deterministic evaluation data, and the same-data guard.
// ---------------------------------------------------------------------------

const norm: CatchUpCalendarEvent = {
  id: 'showing:1',
  title: 'Showing · Villa Mar',
  startAt: '2026-08-26T14:00:00.000Z',
  endAt: '2026-08-26T15:00:00.000Z',
  allDay: false,
  personId: 'p1',
  personName: 'Jane',
  propertyName: 'Villa Mar',
  kind: 'showing',
  source: 'canonical:showing',
}

test('CAL-02: mappers preserve id/title/timestamps for both engines', () => {
  const a = toIlamyCalendarEvent(norm)
  const b = toFullCalendarEvent(norm)
  assert.equal(a.id, 'showing:1')
  assert.equal(b.id, 'showing:1')
  assert.equal(a.title, 'Showing · Villa Mar')
  assert.equal(b.title, 'Showing · Villa Mar')
  assert.equal(a.start, norm.startAt)
  assert.equal(b.start, norm.startAt)
  assert.equal(a.end, norm.endAt)
  assert.equal(b.end, norm.endAt)
  assert.equal(a.allDay, false)
  assert.equal(b.allDay, false)
})

test('CAL-02: ilamy mapper adds color tokens; FullCalendar adds bg/border/text', () => {
  const a = toIlamyCalendarEvent(norm)
  const b = toFullCalendarEvent(norm)
  assert.equal(a.color, '#c6a15b') // showing -> gold
  assert.ok(a.backgroundColor)
  assert.equal(b.backgroundColor, '#c6a15b')
  assert.equal(b.borderColor, '#c6a15b')
  assert.equal(b.textColor, '#ffffff')
})

test('CAL-02: evaluation data includes timed, same-day, all-day, multi-day events', () => {
  const events = getEvaluationCalendarEvents(new Date('2026-08-05T00:00:00Z'))
  assert.equal(events.length, 5)

  const timed = events.filter((e) => !e.allDay)
  assert.ok(timed.length >= 1, 'has a normal timed event')

  const sameDay = events.filter(
    (e) => e.startAt.slice(0, 10) === '2026-08-05',
  )
  assert.ok(sameDay.length >= 2, 'two events on the same day')

  const allDay = events.filter((e) => e.allDay)
  assert.ok(allDay.length >= 1, 'has an all-day event')

  const multiDay = events.filter((e) => {
    const start = new Date(e.startAt).getTime()
    const end = new Date(e.endAt ?? e.startAt).getTime()
    return end - start > 24 * 60 * 60 * 1000
  })
  assert.ok(multiDay.length >= 1, 'has a multi-day event')
  assert.ok(multiDay.some((e) => e.id === 'eval:travel'), 'travel spans days')
})

test('CAL-02: evaluation data is anchored to the current month', () => {
  const base = new Date('2026-08-05T00:00:00Z')
  const events = getEvaluationCalendarEvents(base)
  for (const e of events) {
    const d = new Date(e.startAt)
    assert.equal(d.getUTCMonth(), 7, 'event month matches base month')
    assert.equal(d.getUTCFullYear(), 2026)
  }
})

test('CAL-02: the evaluation passes the SAME events to both candidates (source guard)', () => {
  const src = readFileSync(
    new URL('../../components/portal/catch-up-calendar-evaluation.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/CatchUpCalendar events=\{events\}/.test(src), 'Option A gets events')
  assert.ok(/FullCalendarCandidate events=\{events\}/.test(src), 'Option B gets events')
  assert.ok(!/fetch|getCatchUpCalendarEvents/.test(src), 'no per-candidate data fetch')
})

test('CAL-02: FullCalendar candidate uses the shared engine + mapper (source guard)', () => {
  const src = readFileSync(
    new URL('../../components/portal/fullcalendar-candidate.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/@fullcalendar\/react/.test(src), 'uses FullCalendar')
  assert.ok(/toFullCalendarEvent/.test(src), 'uses the shared mapper')
  assert.ok(/initialView="dayGridMonth"/.test(src), 'defaults to Month view')
})

test('CAL-02: ilamy candidate uses the shared mapper (source guard)', () => {
  const src = readFileSync(
    new URL('../../components/portal/catch-up-calendar.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/toIlamyCalendarEvent/.test(src), 'uses the shared ilamy mapper')
})
