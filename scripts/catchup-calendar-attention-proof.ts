// CATCH-UP DEV proof (Scenario D, calendar_attention) — schedule a showing for
// an eligible person who is NOT a brand-new lead, and confirm the derived reason
// becomes calendar-driven.
import { createShowing, scheduleShowing } from '../db/portal-writes'
import { getCatchUpEligiblePage } from '../db/catch-up'
import { buildCatchUpQueue, toFacts } from '../lib/catchup/queue'
import { deriveAttention } from '../lib/catchup/rules'

async function main() {
  const { rows } = await getCatchUpEligiblePage({ page: 1, pageSize: 600 })
  // A person with an outbound response (not a fresh no-response lead).
  const target = rows.find((r) => r.lastOutboundAt && r.email)
  if (!target) {
    console.log('no eligible person with outbound contact')
    return
  }
  console.log('target =', target.displayName)

  const showing = await createShowing({ personId: target.personId })
  const tomorrow = new Date(Date.now() + 24 * 60 * 60 * 1000)
  const iso = `${tomorrow.toISOString().slice(0, 10)}T14:00:00.000Z`
  await scheduleShowing(showing.id, iso)

  const { rows: pageRows } = await getCatchUpEligiblePage({ page: 1, pageSize: 600 })
  const refreshed = pageRows.find((r) => r.personId === target.personId)
  const attention = refreshed ? deriveAttention(toFacts(refreshed)) : null
  console.log('attention =', JSON.stringify(attention))
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
