// Apple Messages -> EXISTING neutral ODS intake — DEV runtime proof.
// Uses a deterministic synthetic export (the real ~/Library/Messages DB is
// TCC-blocked in this environment), exercises upsert -> reconcile -> evidence,
// proves replay does not duplicate, then cleans up.
//   node --env-file=.env.local --import tsx scripts/apple-messages-intake-proof.ts
import { buildMessagesRelationshipEvidence, APPLE_MESSAGES_SOURCE, type AppleMessagesExport } from '../lib/relationship-intel/apple-messages'
import { upsertRelationshipEvidence, recordReconcileDecision } from '../db/relationship-evidence'
import { reconcileEvidence } from '../lib/relationship-intel/reconcile'
import { createInMemoryPersonLookup } from '../lib/relationship-intel/inmemory-lookup'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''

function syntheticExport(): AppleMessagesExport {
  const handles = [
    { rowid: 1, id: '+17875550134', country: 'us', service: 'iMessage', uncanonicalizedId: '+1 (787) 555-0134', personCentricId: null },
    { rowid: 2, id: 'jane@example.com', country: null, service: 'iMessage', uncanonicalizedId: 'Jane@Example.com', personCentricId: null },
    { rowid: 3, id: '+17875550987', country: 'us', service: 'SMS', uncanonicalizedId: '17875550987', personCentricId: null },
  ]
  const msg = (rowid: number, guid: string, handleId: number, iso: string, isFromMe: number, text: string | null) => ({
    rowid, guid, chatGuid: 'chat-a', handleId, handleValue: String(handleId), service: 'iMessage',
    date: null, dateISO: iso, isFromMe, text, hasAttachments: 0,
  })
  const messages = [
    msg(1, 'm1', 1, '2023-01-02T10:00:00.000Z', 0, 'Hello from Ana'),
    msg(2, 'm2', 1, '2023-01-02T10:05:00.000Z', 1, 'Hi Ana, good to hear'),
    msg(3, 'm3', 1, '2023-01-03T09:00:00.000Z', 0, 'Are you free for a showing?'),
    msg(4, 'm4', 2, '2023-02-01T08:00:00.000Z', 1, 'Sent you the offer docs'),
    msg(5, 'm5', 3, '2023-03-15T18:00:00.000Z', 0, 'SMS from a service number'),
  ]
  return { sourceAccount: 'local_mac_proof', handles, messages }
}

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const exportData = syntheticExport()
    const out = (...a: unknown[]) => console.log(...a)
    const rows = buildMessagesRelationshipEvidence(exportData)
    out('BUILT evidence rows (per handle):', rows.length)

    const { lookup } = await createInMemoryPersonLookup(execute)
    const ids: string[] = []
    for (const { evidence, fingerprint } of rows) {
      const id = await upsertRelationshipEvidence(evidence, fingerprint, undefined, execute)
      ids.push(id)
      const decision = await reconcileEvidence(evidence, lookup)
      await recordReconcileDecision(id, decision, execute)
    }
    out('UPSERTED + reconciled rows:', ids.length)

    const persisted = (await execute`select source, source_account, source_identity_key, review_state, has_email, has_phone, inbound_count, outbound_count, is_two_way, first_observed_at, last_observed_at from integration_relationship_evidence where source = ${APPLE_MESSAGES_SOURCE}`) as Array<Record<string, unknown>>
    out('PERSISTED apple_messages evidence rows:', persisted.length)
    for (const r of persisted) out('  ', r.source_identity_key, '| review=', r.review_state, '| in=', r.inbound_count, 'out=', r.outbound_count, 'twoWay=', r.is_two_way, 'first=', r.first_observed_at, 'last=', r.last_observed_at)

    // Replay: upsert the same evidence again -> no new row (unique source identity).
    const before = persisted.length
    for (const { evidence, fingerprint } of rows) {
      await upsertRelationshipEvidence(evidence, fingerprint, undefined, execute)
    }
    const after = (await execute`select count(*)::int as n from integration_relationship_evidence where source = ${APPLE_MESSAGES_SOURCE}`) as { n: number }[]
    out('REPLAY -> rows before=', before, 'after=', after[0].n, 'no duplicate=', after[0].n === before)

    // Cleanup.
    await execute`delete from integration_relationship_evidence where source = ${APPLE_MESSAGES_SOURCE}`
    out('CLEANUP done.')
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
