// MAC-SYNC-CAL-01 — record live EventKit proof on the Story Board.
import { sql } from '../db/client'
import { createStoryboardStory, updateStoryboardStory } from '../db/storyboard'

const id = 'MAC-SYNC-CAL-01'
const notes = `Live EventKit proof (on Chris's Mac): status=granted, events=31, /tmp/culebraluxe-calendar.json. Default bounded window 7d past / 60d future (the live proof run accidentally used --future-days 600; normal operation uses 7/60 via \`pnpm calendar:sync\`). Wiring: CalendarEventKit.swift -> bounded JSON snapshot -> lib/catchup/eventkit.ts (per-occurrence stable ids, idempotent replay, unlinked personId=null) -> db/catch-up-calendar.ts (merges canonical showings + Apple events) -> CalendarEventSource -> Catch-Up Option A (ilamy). Read/render, showing+Apple merge, and replay idempotency PROVEN (33 events = 2 showings + 31 Apple, 31 distinct ids). EDIT/DELETE live proof: PENDING (requires creating/editing a real test event on the Mac). Not marked Complete/100% until edit/delete proven.`

async function main() {
  const existing = await sql`select id from storyboard_story where id = ${id}`
  const fields = {
    id,
    workstream: 'Integration / Apple / Calendar',
    title: 'Apple Calendar EventKit Adapter — Feed Catch-Up Calendar',
    priority: 'High',
    status: 'In Progress',
    notes,
    batch: null,
    goal: null,
    scope: null,
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: null,
    acceptanceCriteria: null,
    postconditions: null,
    completion: 70,
    rollup: false,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    operatingSurface: 'OPS',
  }

  if (existing.length > 0) {
    await updateStoryboardStory(id, fields)
    console.log('updated', id)
  } else {
    await createStoryboardStory(fields)
    console.log('created', id)
  }
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
