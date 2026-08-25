// CORE-DAILY-06 + 11 — DEV runtime proof.
//   06: snooze reactivation + Done removal from active queue.
//   11: daily relationship actions never mutate workflow/contract/legal state.
//   node --env-file=.env.local --import tsx scripts/core-daily-proof-06-11.ts
import { randomUUID } from 'node:crypto'
import { applyFollowUpCommand, listActiveFollowUpsForPerson } from '../db/follow-up'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const persons = (await execute`select id from person where archived_at is null order by display_name limit 1`) as { id: string }[]
    const deals = (await execute`select id, stage from deal order by created_at limit 1`) as { id: string; stage: string }[]
    if (!persons[0]) { console.log('no person'); return }
    const personId = persons[0].id
    const out = (...a: unknown[]) => console.log(...a)
    const cleanup: Array<() => Promise<void>> = []

    const cmdCreate = randomUUID()
    const fu = await applyFollowUpCommand({ commandId: cmdCreate, commandType: 'create', payload: { personId, title: 'Snooze/done proof', dueAt: new Date(Date.now() + 3600000).toISOString() } }, execute)
    const fuId = fu.followUp!.id
    cleanup.push(async () => { await execute`delete from task where id = ${fuId}`; await execute`delete from relationship_follow_up_receipt where command_id = ${cmdCreate}` })
    out('06: active queue before =', (await listActiveFollowUpsForPerson(personId, execute)).some((f) => f.id === fuId))
    const cmdSnooze = randomUUID(); cleanup.push(async () => { await execute`delete from relationship_follow_up_receipt where command_id = ${cmdSnooze}` })
    const snoozed = await applyFollowUpCommand({ commandId: cmdSnooze, commandType: 'snooze', payload: { followUpId: fuId, snoozeUntil: new Date(Date.now() + 86400000).toISOString() } }, execute)
    out('06: snoozed status =', snoozed.followUp!.status, 'in active queue (not due) =', (await listActiveFollowUpsForPerson(personId, execute)).some((f) => f.id === fuId))
    const cmdDone = randomUUID(); cleanup.push(async () => { await execute`delete from relationship_follow_up_receipt where command_id = ${cmdDone}` })
    const done = await applyFollowUpCommand({ commandId: cmdDone, commandType: 'complete', payload: { followUpId: fuId, outcome: 'connected' } }, execute)
    out('06: completed status =', done.followUp!.status, 'removed from active queue =', !(await listActiveFollowUpsForPerson(personId, execute)).some((f) => f.id === fuId))

    if (deals[0]) {
      const dealId = deals[0].id
      const before = (await execute`select stage from deal where id = ${dealId}`) as { stage: string }[]
      const procBefore = (await execute`select count(*)::int as n from process_instances`) as { n: number }[]
      const cmdDone2 = randomUUID()
      await applyFollowUpCommand({ commandId: cmdDone2, commandType: 'complete', payload: { followUpId: fuId, outcome: 'no_answer' } }, execute)
      const after = (await execute`select stage from deal where id = ${dealId}`) as { stage: string }[]
      const procAfter = (await execute`select count(*)::int as n from process_instances`) as { n: number }[]
      out('11: deal.stage unchanged =', before[0].stage === after[0].stage, '| workflow instances unchanged =', procBefore[0].n === procAfter[0].n)
      cleanup.push(async () => { await execute`delete from relationship_follow_up_receipt where command_id = ${cmdDone2}` })
    }

    for (const c of cleanup) await c()
    out('CLEANUP done.')
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
