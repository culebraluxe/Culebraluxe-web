// MAC-SYNC-CAL-01 — DEV seam proof. Verifies that EventKit-normalized snapshot
// events (as the Swift bridge emits) flow through the CalendarEventSource seam
// into Catch-Up, idempotently. Uses a clearly-labeled DEV sample (not real
// Apple sync — EventKit TCC is denied in headless shells).
import { writeFileSync } from 'node:fs'

const SAMPLE = [
  {
    eventIdentifier: 'EK-DEV-SAMPLE-001',
    sourceAccount: 'apple-calendar:local',
    calendarName: 'Personal',
    title: 'Meeting · Buyer Susan',
    startAt: '2026-08-27T16:00:00.000Z',
    endAt: '2026-08-27T17:00:00.000Z',
    allDay: false,
    location: 'Culebra',
    notes: null,
  },
  {
    eventIdentifier: 'EK-DEV-SAMPLE-002',
    sourceAccount: 'apple-calendar:icloud',
    calendarName: 'Work',
    title: 'Showing · Casa Mar',
    startAt: '2026-08-28T10:00:00.000Z',
    endAt: '2026-08-28T11:00:00.000Z',
    allDay: false,
    location: null,
    notes: null,
  },
  {
    eventIdentifier: 'EK-DEV-SAMPLE-003',
    sourceAccount: 'apple-calendar:icloud',
    calendarName: 'Personal',
    title: 'Mainland trip',
    startAt: '2026-08-30T00:00:00.000Z',
    endAt: '2026-09-02T00:00:00.000Z',
    allDay: true,
    location: null,
    notes: null,
  },
]

async function main() {
  const samplePath = '/tmp/culebraluxe-calendar-dev-sample.json'
  writeFileSync(samplePath, JSON.stringify(SAMPLE))

  process.env.MAC_BRIDGE_CALENDAR_JSON = samplePath
  const { getCatchUpCalendarEvents } = await import('../db/catch-up-calendar')

  const events = await getCatchUpCalendarEvents()
  const apple = events.filter((e) => e.source === 'apple_calendar')
  console.log('total calendar events:', events.length)
  console.log('apple_calendar events:', apple.length)
  console.log(JSON.stringify(apple, null, 2))

  const again = await getCatchUpCalendarEvents()
  const ids1 = events.map((e) => e.id).sort()
  const ids2 = again.map((e) => e.id).sort()
  console.log('idempotent across loads:', JSON.stringify(ids1) === JSON.stringify(ids2))
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
