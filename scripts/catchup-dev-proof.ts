// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// CATCH-UP real DEV proof — creates website leads via the canonical intake and
// confirms they surface in the derived Catch-Up queue. DEV only (authorized by
// the work order's "REAL DEV PROOF — create it").
import { createWebsiteLead } from '@/legacy/db/catchup-lead'
import { getCatchUpEligiblePage } from '@/legacy/db/catch-up'
import { buildCatchUpQueue } from '../lib/catchup/queue'

async function main() {
  const stamp = Date.now()

  // SCENARIO A — Name + Email
  const emailLead = await createWebsiteLead({
    name: `Proof Lead ${stamp}`,
    email: `proof${stamp}@example.com`,
    message: 'Interested in beachfront homes.',
  })

  // SCENARIO A (repeat) — Name + Phone
  const phoneLead = await createWebsiteLead({
    name: `Proof Phone Lead ${stamp}`,
    phone: `+1787${String(stamp).slice(-7)}`,
  })

  console.log('emailLead =', JSON.stringify(emailLead))
  console.log('phoneLead =', JSON.stringify(phoneLead))

  // Confirm both surface in the derived queue.
  const { rows, total } = await getCatchUpEligiblePage({ page: 1, pageSize: 50 })
  const queue = buildCatchUpQueue(rows)
  const found = queue.filter(
    (i) => i.personId === emailLead.personId || i.personId === phoneLead.personId,
  )

  console.log('eligible total =', total)
  console.log('all rows =', JSON.stringify(rows, null, 2))
  console.log('queue found =', JSON.stringify(found, null, 2))
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
