// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The module this file lazily imports was deleted in the 2026-09 Rust port, so the file
// still parses but cannot do its job: the code path it needs is gone. Kept as REFERENCE
// ONLY, so the behaviour can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// MAC-SYNC-CAL-01 — live DEV seam proof. Reads the REAL EventKit snapshot
// (default /tmp/culebraluxe-calendar.json, the Mac bridge output) through the
// CalendarEventSource seam and reports aggregate structure only — private
// titles/notes are never printed. Verifies distinct per-occurrence ids,
// idempotent replay, and showing + Apple merge.
async function main() {
  const { getCatchUpCalendarEvents } = await import('@/legacy/db/catch-up-calendar')
  const { eventKitSnapshotPath } = await import('../lib/catchup/eventkit')

  const events = await getCatchUpCalendarEvents()
  const apple = events.filter((e) => e.source === 'apple_calendar')
  const showing = events.filter((e) => e.source === 'canonical:showing')
  console.log('snapshot path:', eventKitSnapshotPath())
  console.log('total calendar events:', events.length)
  console.log('apple_calendar events:', apple.length)
  console.log('canonical showing events:', showing.length)
  console.log('distinct apple ids:', new Set(apple.map((e) => e.id)).size)
  console.log('apple kinds:', JSON.stringify([...new Set(apple.map((e) => e.kind))]))
  console.log('apple all-day count:', apple.filter((e) => e.allDay).length)
  console.log('apple all unlinked:', apple.every((e) => e.personId === null))

  const again = await getCatchUpCalendarEvents()
  const ids1 = events.map((e) => e.id).sort()
  const ids2 = again.map((e) => e.id).sort()
  console.log('idempotent across loads:', JSON.stringify(ids1) === JSON.stringify(ids2))
  console.log('private titles/notes NOT logged (this script only reports aggregates)')
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
