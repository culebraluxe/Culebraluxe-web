// CORE-DAILY-09 + 10 — DEV runtime proof.
//   09: Client daily flow — outcome capture (Interaction), quick next action,
//       contact evidence, replay-safe.
//   10: Contract operating summary from authoritative Deal facts + next action.
//   node --env-file=.env.local --import tsx scripts/core-daily-proof-09-10.ts
import { randomUUID } from 'node:crypto'
import { recordContactOutcome, applyFollowUpCommand } from '../db/follow-up'
import { getDealWorkspace } from '../db/deal-workspace'
import { getContactEvidence } from '../db/attention'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const persons = (await execute`select id from person where archived_at is null order by display_name limit 1`) as { id: string }[]
    const deals = (await execute`select id from deal order by created_at limit 1`) as { id: string }[]
    if (!persons[0]) { console.log('no person'); return }
    const personId = persons[0].id
    const out = (...a: unknown[]) => console.log(...a)
    const clean: Array<() => Promise<void>> = []

    // ---- 09: Client flow ----
    const cmdOutcome = randomUUID()
    const o = await recordContactOutcome({ commandId: cmdOutcome, personId, channel: 'call', outcome: 'connected', source: 'client_manager' }, execute)
    clean.push(async () => { await execute`delete from interaction where source_system='relationship_follow_up' and source_external_id=${cmdOutcome}`; await execute`delete from relationship_follow_up_receipt where command_id=${cmdOutcome}` })
    out('09: Client outcome -> Interaction written =', o.interactionId !== null)
    const intCount = (await execute`select count(*)::int as n from interaction where source_external_id=${cmdOutcome}`) as { n: number }[]
    const oReplay = await recordContactOutcome({ commandId: cmdOutcome, personId, channel: 'call', outcome: 'connected' }, execute)
    const intCount2 = (await execute`select count(*)::int as n from interaction where source_external_id=${cmdOutcome}`) as { n: number }[]
    out('09: replay -> duplicate=', oReplay.duplicate, '| Interaction rows =', intCount2[0].n, '(must be', intCount[0].n, ')')

    const cmdNext = randomUUID()
    const n = await applyFollowUpCommand({ commandId: cmdNext, commandType: 'create', payload: { personId, title: 'Check financing', source: 'client_manager' } }, execute)
    clean.push(async () => { await execute`delete from task where id=${n.followUp!.id}`; await execute`delete from relationship_follow_up_receipt where command_id=${cmdNext}` })
    out('09: quick next action (Client) -> follow-up created =', Boolean(n.followUp!.id), 'status=', n.followUp!.status)

    const ce = await getContactEvidence([personId], execute)
    out('09: contact evidence present =', Boolean(ce[personId] && (ce[personId].emails.length > 0 || ce[personId].phones.length > 0)))

    // ---- 10: Contract flow ----
    if (deals[0]) {
      const dealId = deals[0].id
      const ws = await getDealWorkspace(dealId)
      out('10: Contract summary -> stage=', ws.deal?.stage, '| property=', ws.property?.name ?? null, '| client=', ws.client?.displayName ?? null)
      out('10: material dates -> closing=', ws.deal?.closingDateLabel ?? 'not recorded', '| created=', ws.deal?.createdAtLabel)
      out('10: next action (open tasks) =', ws.openTasks.length > 0 ? ws.openTasks[0].title : 'none')
      out('10: offer state -> offers=', ws.offers.length, '| latest=', ws.offers[ws.offers.length - 1]?.status ?? 'none')
      const cmdDealNext = randomUUID()
      const d = await applyFollowUpCommand({ commandId: cmdDealNext, commandType: 'create', payload: { personId, dealId, title: 'Check appraisal', source: 'contract_summary' } }, execute)
      clean.push(async () => { await execute`delete from task where id=${d.followUp!.id}`; await execute`delete from relationship_follow_up_receipt where command_id=${cmdDealNext}` })
      out('10: quick action (Contract) -> follow-up created with dealId linked =', d.followUp!.dealId === dealId)
    }

    for (const c of clean) await c()
    out('CLEANUP done.')
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
