import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  mapProjectCalendarItems,
  mapProjectCalendarToEvents,
} from '../ui/projects/secondary-projection'
import type { WbsItem } from '../services/wbs'

const item = (id: string, dueAt: string | null): WbsItem => ({
  id, title: id, notes: '', category: 'clients', status: 'open', projectId: 'p', parentId: null,
  dueAt, owner: null, order: null, entity: null, createdAt: null, updatedAt: null,
})

test('project calendar projection includes only valid persisted WBS due dates in stable order', () => {
  const result = mapProjectCalendarItems([
    item('later', '2026-09-12T00:00:00.000Z'),
    item('no-date', null),
    item('bad-date', 'not-a-date'),
    item('earlier', '2026-09-10T00:00:00.000Z'),
  ])
  assert.deepEqual(result.map((entry) => entry.id), ['earlier', 'later'])
})

test('month-view events carry the project WBS dates as all-day events, and invent no semantics', () => {
  const items = mapProjectCalendarItems([
    item('later', '2026-09-12T00:00:00.000Z'),
    item('no-date', null),
    item('earlier', '2026-09-10T00:00:00.000Z'),
  ])
  const events = mapProjectCalendarToEvents(items)

  assert.deepEqual(events.map((e) => e.id), ['earlier', 'later'])
  assert.deepEqual(events.map((e) => e.startAt), [
    '2026-09-10T00:00:00.000Z',
    '2026-09-12T00:00:00.000Z',
  ])
  // A due date is a date, not a time: every event is all-day.
  assert.ok(events.every((e) => e.allDay))
  // No invented kind (a deadline is not a showing/meeting/call) and no invented identity.
  assert.ok(events.every((e) => e.kind === 'other'))
  assert.ok(events.every((e) => e.source === 'wbs'))
  assert.ok(events.every((e) => e.personId === null && e.personName === null && e.propertyName === null))
})

test('month-view events survive an empty project without fabricating rows', () => {
  assert.deepEqual(mapProjectCalendarToEvents([]), [])
  assert.deepEqual(mapProjectCalendarToEvents(mapProjectCalendarItems([item('no-date', null)])), [])
})
