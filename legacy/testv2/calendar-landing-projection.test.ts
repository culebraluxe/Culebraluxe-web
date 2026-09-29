import { test } from 'node:test'
import assert from 'node:assert/strict'

import { landingCalendarRowsToCatchUp } from '@/legacy/db/catch-up-calendar'

test('calendar landing: projects a durable Apple event without turning it into work', () => {
  const events = landingCalendarRowsToCatchUp([
    {
      id: 'landing-1',
      source_account: 'icloud-account',
      source_message_id: 'eventkit:OCC-123',
      title: 'Broker check-in',
      starts_at: '2026-09-14T14:00:00.000Z',
      ends_at: '2026-09-14T15:00:00.000Z',
      all_day: false,
    },
  ])

  assert.equal(events.length, 1)
  assert.equal(events[0]?.id, 'eventkit:OCC-123')
  assert.equal(events[0]?.title, 'Broker check-in')
  assert.equal(events[0]?.source, 'apple_calendar')
  assert.equal(events[0]?.kind, 'meeting')
  assert.equal(events[0]?.personId, null)
  assert.equal(events[0]?.propertyName, null)
})

test('calendar landing: all-day facts stay calendar events and malformed undated rows are ignored', () => {
  const events = landingCalendarRowsToCatchUp([
    {
      id: 'landing-all-day',
      source_account: 'icloud-account',
      source_message_id: 'eventkit:ALLDAY-1',
      title: 'Travel',
      starts_at: '2026-09-14T04:00:00.000Z',
      ends_at: '2026-09-15T04:00:00.000Z',
      all_day: true,
    },
    {
      id: 'landing-undated',
      source_account: 'icloud-account',
      source_message_id: 'eventkit:BAD',
      title: 'Bad row',
      starts_at: null,
      ends_at: null,
      all_day: false,
    },
  ])

  assert.equal(events.length, 1)
  assert.equal(events[0]?.allDay, true)
  assert.equal(events[0]?.kind, 'other')
})
