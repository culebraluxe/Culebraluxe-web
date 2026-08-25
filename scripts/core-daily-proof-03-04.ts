// CORE-DAILY-03 + 04 — DEV runtime proof.
// Outcome capture (Interaction + follow-up complete + next touch, replay-safe)
// and quick next-action presets (canonical follow-up create, replay-safe).
//   node --env-file=.env.local --import tsx scripts/core-daily-proof-03-04.ts
import { randomUUID } from 'node:crypto'
import {
  applyFollowUpCommand,
  recordContactOutcome,
} from '../db/follow-up'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const persons = (await execute`select id from person where archived_at is null order by display_name limit 1`) as { id: string }[]
    const deals = (await execute`select id from deal order by created_at limit 1`) as { id: string }[]
    if (!persons[0]) { console.log('no person'); return }
    const personId = persons[0].id
    const dealId = deals[0]?.id ?? null
    const createdTaskIds: string[] = []
    const commandIds: string[] = []
    const out = (...a: unknown[]) => console.log(...a)

    // Create a due follow-up to complete via an outcome.
    const cmdCreate = randomUUID(); commandIds.push(cmdCreate)
    const fu = await applyFollowUpCommand({ commandId: cmdCreate, commandType: 'create', payload: { personId, title: 'Call about showing', dueAt: new Date().toISOString() } }, execute)
    createdTaskIds.push(fu.followUp!.id)

    // OUTCOME: connected -> Interaction + follow-up completed + correlation.
    const cmdOutcome = randomUUID(); commandIds.push(cmdOutcome)
    const o1 = await recordContactOutcome({
      commandId: cmdOutcome, personId, channel: 'call', outcome: 'connected',
      followUpId: fu.followUp!.id, title: 'Connected with Ana',
    }, execute)
    out('OUTCOME connected -> interactionId=', Boolean(o1.interactionId), 'followUp completed=', Boolean(o1.followUpId))
    const fx = (await execute`select id, status, outcome, source_interaction_id from task where id = ${fu.followUp!.id}`) as { id: string; status: string; outcome: string; source_interaction_id: string | null }[]
    out('  follow-up after outcome -> status=', fx[0].status, 'outcome=', fx[0].outcome, 'correlated interaction=', Boolean(fx[0].source_interaction_id))
    const intRows = (await execute`select id, channel, event_type, direction, source_system, source_external_id from interaction where source_external_id = ${cmdOutcome}`) as { id: string; channel: string; event_type: string; direction: string; source_system: string; source_external_id: string }[]
    out('  Interaction row -> channel=', intRows[0].channel, 'event_type=', intRows[0].event_type, 'direction=', intRows[0].direction, 'source_system=', intRows[0].source_system)

    // REPLAY outcome -> duplicate, no duplicate Interaction.
    const o1r = await recordContactOutcome({ commandId: cmdOutcome, personId, channel: 'call', outcome: 'connected' }, execute)
    out('REPLAY outcome -> duplicate=', o1r.duplicate)
    const intDup = (await execute`select count(*)::int as n from interaction where source_external_id = ${cmdOutcome}`) as { n: number }[]
    out('  Interaction rows for command =', intDup[0].n, '(must be 1)')

    // OUTCOME no_answer + next touch -> exactly one next obligation.
    const cmdNoAnswer = randomUUID(); commandIds.push(cmdNoAnswer)
    const o2 = await recordContactOutcome({
      commandId: cmdNoAnswer, personId, channel: 'call', outcome: 'no_answer',
      nextTouchAt: new Date(Date.now() + 86400000).toISOString(), nextTouchTitle: 'Call back Ana',
    }, execute)
    createdTaskIds.push(o2.nextFollowUpId!)
    out('OUTCOME no_answer + next touch -> nextFollowUp created=', Boolean(o2.nextFollowUpId))

    // REPLAY no_answer+next -> no duplicate next.
    const o2r = await recordContactOutcome({
      commandId: cmdNoAnswer, personId, channel: 'call', outcome: 'no_answer',
      nextTouchAt: new Date().toISOString(), nextTouchTitle: 'Call back Ana',
    }, execute)
    out('REPLAY no_answer+next -> duplicate=', o2r.duplicate, 'no new next=', o2r.nextFollowUpId === null)
    const nextCount = (await execute`select count(*)::int as n from task where reason = 'next_touch_after_outcome'`) as { n: number }[]
    out('  next_touch_after_outcome tasks =', nextCount[0].n)

    // QUICK NEXT ACTION from person context.
    const cmdNext = randomUUID(); commandIds.push(cmdNext)
    const q1 = await applyFollowUpCommand({ commandId: cmdNext, commandType: 'create', payload: { personId, title: 'Check financing', source: 'quick_next_action' } }, execute)
    createdTaskIds.push(q1.followUp!.id)
    out('QUICK next action (person) -> created=', Boolean(q1.followUp!.id), 'status=', q1.followUp!.status)
    // QUICK NEXT ACTION from deal context.
    if (dealId) {
      const cmdNextDeal = randomUUID(); commandIds.push(cmdNextDeal)
      const q2 = await applyFollowUpCommand({ commandId: cmdNextDeal, commandType: 'create', payload: { personId, dealId, title: 'Check appraisal', source: 'quick_next_action' } }, execute)
      createdTaskIds.push(q2.followUp!.id)
      out('QUICK next action (deal) -> created=', Boolean(q2.followUp!.id), 'dealId linked=', q2.followUp!.dealId === dealId)
      const q2r = await applyFollowUpCommand({ commandId: cmdNextDeal, commandType: 'create', payload: { personId, dealId, title: 'Check appraisal' } }, execute)
      out('REPLAY deal next action -> duplicate=', q2r.duplicate)
    }

    const receipts = (await execute`select command_id, command_type, applied from relationship_follow_up_receipt where command_id = any (${commandIds})`) as { command_id: string; command_type: string; applied: boolean }[]
    out('RECEIPTS persisted =', receipts.length, 'all applied =', receipts.every((r) => r.applied))

    await execute`delete from interaction where source_system = 'relationship_follow_up' and source_external_id = any (${commandIds})`
    await execute`delete from task where id = any (${createdTaskIds})`
    await execute`delete from relationship_follow_up_receipt where command_id = any (${commandIds})`
    out('CLEANUP done.')
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })

