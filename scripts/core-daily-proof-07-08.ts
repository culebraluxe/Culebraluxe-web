// CORE-DAILY-07 + 08 — DEV runtime proof: deterministic recommendations + suppression.
// Seeds an overdue follow-up, verifies the recommendation appears with an
// explanation, dismisses it, verifies it stays suppressed, then cleans up.
//   node --env-file=.env.local --import tsx scripts/core-daily-proof-07-08.ts
import { randomUUID } from 'node:crypto'
import { applyFollowUpCommand } from '../db/follow-up'
import { getRecommendations, dismissRecommendation } from '../db/recommendations'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const persons = (await execute`select id from person where archived_at is null order by display_name limit 1`) as { id: string }[]
    if (!persons[0]) { console.log('no person'); return }
    const personId = persons[0].id
    const out = (...a: unknown[]) => console.log(...a)

    // Seed an OVERDUE follow-up.
    const cmdCreate = randomUUID()
    const fu = await applyFollowUpCommand({
      commandId: cmdCreate, commandType: 'create',
      payload: { personId, title: 'Overdue call', dueAt: new Date(Date.now() - 86400000).toISOString(), source: 'recommendation_proof' },
    }, execute)
    out('SEED overdue follow-up -> id=', fu.followUp!.id, 'dueAt set=', Boolean(fu.followUp!.dueAt))

    // Recommendations include the overdue commitment with an explanation.
    const recs1 = await getRecommendations(execute)
    const rec = recs1.find((r) => r.personId === personId && r.code === 'overdue_relationship_commitment')
    out('RECOMMENDATION appears=', Boolean(rec))
    out('  reason=', rec?.reason, '| explanationCode=', rec?.explanationCode, '| evidencePointer=', rec?.evidencePointers[0])

    // Dismiss -> suppressed on regeneration.
    await dismissRecommendation(personId, 'overdue_relationship_commitment', null, execute)
    const recs2 = await getRecommendations(execute)
    const rec2 = recs2.find((r) => r.personId === personId && r.code === 'overdue_relationship_commitment')
    out('DISMISSED stays suppressed=', rec2 === undefined)
    // Regenerate again -> still suppressed (no multiplication).
    const recs3 = await getRecommendations(execute)
    out('REGENERATION count (suppressed) =', recs3.filter((r) => r.personId === personId && r.code === 'overdue_relationship_commitment').length)

    // Cleanup.
    await execute`delete from relationship_recommendation_dismissal where person_id = ${personId} and code = 'overdue_relationship_commitment'`
    await execute`delete from task where id = ${fu.followUp!.id}`
    await execute`delete from relationship_follow_up_receipt where command_id = ${cmdCreate}`
    out('CLEANUP done.')
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
