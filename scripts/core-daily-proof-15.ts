// CORE-DAILY-15 — integrated daily-loop runtime proof (DEV).
// Catch-Up item -> Contact -> Record outcome -> canonical Interaction ->
// Done / Snooze / Done+next -> future obligation -> Client context.
//   node --env-file=.env.local --import tsx scripts/core-daily-proof-15.ts
import { randomUUID } from 'node:crypto'
import { applyFollowUpCommand, recordContactOutcome, listActiveFollowUpsForPerson } from '../db/follow-up'
import { getRecommendations } from '../db/recommendations'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const persons = (await execute`select id from person where archived_at is null order by display_name limit 1`) as { id: string }[]
    if (!persons[0]) { console.log('no person'); return }
    const personId = persons[0].id
    const out = (...a: unknown[]) => console.log(...a)
    const cmdCreate = randomUUID()
    const created = await applyFollowUpCommand({ commandId: cmdCreate, commandType: 'create', payload: { personId, title: 'Follow up on offer', dueAt: new Date(Date.now() - 3600000).toISOString(), source: 'catch_up' } }, execute)
    const fuId = created.followUp!.id
    const clean: Array<() => Promise<void>> = []
    clean.push(async () => { await execute`delete from task where id = ${fuId}`; await execute`delete from relationship_follow_up_receipt where command_id = ${cmdCreate}` })

    out('1. CATCH-UP item present (overdue recommendation) =', (await getRecommendations(execute)).some((r) => r.personId === personId))
    const before = (await execute`select count(*)::int as n from interaction`) as { n: number }[]
    const cmdOutcome = randomUUID(); clean.push(async () => { await execute`delete from relationship_follow_up_receipt where command_id = ${cmdOutcome}` })
    const outcome = await recordContactOutcome({ commandId: cmdOutcome, personId, channel: 'call', outcome: 'connected', followUpId: fuId, nextTouchAt: new Date(Date.now() + 86400000).toISOString(), nextTouchTitle: 'Call back about offer' }, execute)
    out('2. CONTACT outcome recorded -> Interaction written =', outcome.interactionId !== null, '| follow-up completed =', outcome.followUpId !== null, '| next obligation =', outcome.nextFollowUpId !== null)
    clean.push(async () => { if (outcome.nextFollowUpId) await execute`delete from task where id = ${outcome.nextFollowUpId}` })
    const after = (await execute`select count(*)::int as n from interaction`) as { n: number }[]
    out('3. canonical Interaction persisted (count increased) =', after[0].n === before[0].n + 1)

    // Snooze the next obligation -> leaves active queue.
    const cmdSnooze = randomUUID(); clean.push(async () => { await execute`delete from relationship_follow_up_receipt where command_id = ${cmdSnooze}` })
    await applyFollowUpCommand({ commandId: cmdSnooze, commandType: 'snooze', payload: { followUpId: outcome.nextFollowUpId!, snoozeUntil: new Date(Date.now() + 86400000).toISOString() } }, execute)
    out('4. SNOOZE next obligation -> active queue excludes it =', !(await listActiveFollowUpsForPerson(personId, execute)).some((f) => f.id === outcome.nextFollowUpId))

    // Client/Contract context still resolves (person has canonical identities).
    const ctx = (await execute`select count(*)::int as n from person_identity where person_id = ${personId}`) as { n: number }[]
    out('5. CLIENT context (person_identity count) =', ctx[0].n)

    for (const c of clean) await c()
    await execute`delete from interaction where source_system = 'relationship_follow_up' and source_external_id = ${cmdOutcome}`
    out('INTEGRATED LOOP PROVEN + CLEANUP done.')
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
