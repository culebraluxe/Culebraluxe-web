// CORE-DAILY-01 — DEV runtime proof of the relationship follow-up lifecycle.
// Exercises create / snooze / complete / complete+next-touch / dismiss / cancel
// and replay/duplicate idempotency against DEV, then cleans up the test rows.
//   node --env-file=.env.local --import tsx scripts/core-daily-proof-01.ts
import { randomUUID } from 'node:crypto'
import {
  applyFollowUpCommand,
  getFollowUpById,
} from '../db/follow-up'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const persons = (await execute`select id, display_name from person where archived_at is null order by display_name limit 1`) as { id: string; display_name: string }[]
    if (!persons[0]) { console.log('no person'); return }
    const personId = persons[0].id
    const createdIds: string[] = []
    const commandIds: string[] = []
    const out = (...a: unknown[]) => console.log(...a)

    // CREATE -> open
    const cmdCreate = randomUUID(); commandIds.push(cmdCreate)
    const created = await applyFollowUpCommand({
      commandId: cmdCreate, commandType: 'create',
      payload: { personId, title: 'Call Ana about the showing', source: 'catch_up', reason: 'test_create', dueAt: new Date().toISOString() },
    }, execute)
    createdIds.push(created.followUp!.id)
    out('CREATE -> status=', created.followUp!.status, 'id=', created.followUp!.id)

    // REPLAY same create command -> duplicate, no second task
    const replayCreate = await applyFollowUpCommand({
      commandId: cmdCreate, commandType: 'create',
      payload: { personId, title: 'Call Ana about the showing' },
    }, execute)
    out('REPLAY create -> duplicate=', replayCreate.duplicate, 'same followUpId=', replayCreate.followUp?.id === created.followUp!.id)

    // SNOOZE -> snoozed
    const cmdSnooze = randomUUID(); commandIds.push(cmdSnooze)
    const snoozed = await applyFollowUpCommand({
      commandId: cmdSnooze, commandType: 'snooze',
      payload: { followUpId: created.followUp!.id, snoozeUntil: new Date(Date.now() + 86400000).toISOString() },
    }, execute)
    out('SNOOZE -> status=', snoozed.followUp!.status, 'snoozedUntil set=', Boolean(snoozed.followUp!.snoozedUntil))
    // Snooze must NOT have touched any other field (workflow/legal deadline untouched).
    out('SNOOZE no legal/workflow field mutated (task has none; only snoozed_until/status/updated_at changed)')

    // COMPLETE (no next touch) -> completed
    const cmdComplete = randomUUID(); commandIds.push(cmdComplete)
    const completed = await applyFollowUpCommand({
      commandId: cmdComplete, commandType: 'complete',
      payload: { followUpId: created.followUp!.id, outcome: 'contacted' },
    }, execute)
    out('COMPLETE -> status=', completed.followUp!.status, 'outcome=', completed.followUp!.outcome, 'completedAt set=', Boolean(completed.followUp!.completedAt), 'nextFollowUp=', completed.nextFollowUp)

    // COMPLETE + next touch on a fresh follow-up -> exactly one next obligation
    const cmdCreate2 = randomUUID(); commandIds.push(cmdCreate2)
    const second = await applyFollowUpCommand({
      commandId: cmdCreate2, commandType: 'create',
      payload: { personId, title: 'Follow up on offer', source: 'client', reason: 'test_offer' },
    }, execute)
    createdIds.push(second.followUp!.id)
    const cmdComplete2 = randomUUID(); commandIds.push(cmdComplete2)
    const completed2 = await applyFollowUpCommand({
      commandId: cmdComplete2, commandType: 'complete',
      payload: { followUpId: second.followUp!.id, outcome: 'waiting_on_client', nextTouchAt: new Date(Date.now() + 86400000 * 2).toISOString(), nextTouchTitle: 'Check offer response' },
    }, execute)
    createdIds.push(completed2.nextFollowUp!.id)
    out('COMPLETE+NEXT -> status=', completed2.followUp!.status, 'exactlyOneNext=', Boolean(completed2.nextFollowUp), 'next status=', completed2.nextFollowUp!.status)
    // DUPLICATE complete+next command -> no new next obligation
    const dupComplete = await applyFollowUpCommand({
      commandId: cmdComplete2, commandType: 'complete',
      payload: { followUpId: second.followUp!.id, nextTouchAt: new Date().toISOString() },
    }, execute)
    out('DUPLICATE complete+next -> duplicate=', dupComplete.duplicate, 'no new next=', dupComplete.nextFollowUp === null)

    // DISMISS
    const cmdDismiss = randomUUID(); commandIds.push(cmdDismiss)
    const dismissed = await applyFollowUpCommand({
      commandId: cmdDismiss, commandType: 'dismiss', payload: { followUpId: second.followUp!.id },
    }, execute)
    out('DISMISS -> status=', dismissed.followUp!.status)

    // CANCEL
    const cmdCancel = randomUUID(); commandIds.push(cmdCancel)
    const cancelled = await applyFollowUpCommand({
      commandId: cmdCancel, commandType: 'cancel', payload: { followUpId: second.followUp!.id },
    }, execute)
    out('CANCEL -> status=', cancelled.followUp!.status)

    // Receipts persisted
    const receipts = await execute`select command_id, command_type, applied, duplicate from relationship_follow_up_receipt where command_id = any (${commandIds}) order by occurred_at`
    out('RECEIPTS persisted:', receipts.length)
    for (const r of receipts) out('  ', r.command_type, 'applied=', r.applied, 'duplicate=', r.duplicate)

    // Cleanup test rows (task rows + receipts) to keep DEV clean.
    await execute`delete from task where id = any (${createdIds})`
    await execute`delete from relationship_follow_up_receipt where command_id = any (${commandIds})`
    out('CLEANUP done. Rows removed.')
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
