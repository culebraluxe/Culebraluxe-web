import { readFile } from 'node:fs/promises'

import { landCalendarEvent } from '@/legacy/db/landing'
import {
  eventKitToCatchUpEvent,
  type EventKitNormalizedEvent,
} from '../lib/catchup/eventkit'

const snapshotPath = process.argv[2] || process.env.MAC_BRIDGE_CALENDAR_JSON || '/tmp/culebraluxe-calendar.json'

async function main() {
  const rawText = await readFile(snapshotPath, 'utf8')
  const parsed: unknown = JSON.parse(rawText)
  if (!Array.isArray(parsed)) {
    throw new Error(`Calendar snapshot is not an array: ${snapshotPath}`)
  }

  let inserted = 0
  let replayed = 0
  let rejected = 0

  for (const candidate of parsed) {
    const raw = candidate as EventKitNormalizedEvent
    const event = eventKitToCatchUpEvent(raw)
    if (!event) {
      rejected += 1
      continue
    }

    const wasInserted = await landCalendarEvent({
      sourceAccount: raw.sourceAccount?.trim() || 'apple-calendar',
      sourceMessageId: event.id,
      title: raw.title,
      startsAt: event.startAt,
      endsAt: event.endAt,
      allDay: event.allDay,
      location: raw.location ?? null,
      organizer: null,
      attendees: null,
      raw,
    })

    if (wasInserted) inserted += 1
    else replayed += 1
  }

  // Aggregate-only operational output: never print calendar titles/notes.
  console.log(JSON.stringify({
    source: 'apple_eventkit',
    snapshotPath,
    received: parsed.length,
    inserted,
    replayed,
    rejected,
  }))
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
