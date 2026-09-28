// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// CATCH-UP real DEV proof (Scenario D) — schedule a showing for a proof lead
// and confirm it appears in the calendar projection + drives attention.
import { createShowing, scheduleShowing } from '@/legacy/db/portal-writes'
import { getCatchUpCalendarEvents } from '@/legacy/db/catch-up-calendar'
import { getCatchUpEligiblePage } from '@/legacy/db/catch-up'
import { buildCatchUpQueue } from '../lib/catchup/queue'

async function main() {
  const { rows } = await getCatchUpEligiblePage({
    search: 'Proof Lead',
    page: 1,
    pageSize: 10,
  })
  const lead = rows[0]
  if (!lead) {
    console.log('no proof lead')
    return
  }

  const showing = await createShowing({ personId: lead.personId })
  // Tomorrow 10:00 AM Puerto Rico (UTC-4) = 14:00Z.
  const tomorrow = new Date(Date.now() + 24 * 60 * 60 * 1000)
  const iso = `${tomorrow.toISOString().slice(0, 10)}T14:00:00.000Z`
  const scheduled = await scheduleShowing(showing.id, iso)
  console.log('scheduled showing =', JSON.stringify(scheduled))

  const events = await getCatchUpCalendarEvents()
  console.log('calendar events total =', events.length)
  console.log(
    'matching event =',
    JSON.stringify(events.filter((e) => e.personId === lead.personId), null, 2),
  )

  const { rows: pageRows } = await getCatchUpEligiblePage({ page: 1, pageSize: 50 })
  const queue = buildCatchUpQueue(pageRows)
  const item = queue.find((i) => i.personId === lead.personId)
  console.log(
    'attention =',
    item
      ? { reasonCode: item.reasonCode, reasonLabel: item.reasonLabel }
      : null,
  )
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
