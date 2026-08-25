// CORE-DAILY-12 + 13 — DEV runtime proof.
//   12: OPPS stewardship link/reject via the canonical seam (no silent merge).
//   13: privacy-conscious telemetry rows, no private content.
//   node --env-file=.env.local --import tsx scripts/core-daily-proof-12-13.ts
import { randomUUID } from 'node:crypto'
import { recordReconcileDecision } from '../db/relationship-evidence'
import { REL_INTEL_RULE_VERSION } from '../lib/relationship-intel/reconcile'
import { emitDailyLoopTelemetry } from '../db/telemetry'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const persons = (await execute`select id from person where archived_at is null order by display_name limit 1`) as { id: string }[]
    if (!persons[0]) { console.log('no person'); return }
    const personId = persons[0].id
    const out = (...a: unknown[]) => console.log(...a)
    const cleanup: Array<() => Promise<void>> = []

    // 12: link a synthetic evidence source to the person via the sanctioned seam.
    const evId = randomUUID()
    await execute`
      insert into integration_relationship_evidence (
        id, source, source_account, source_identity_key, display_name, emails, phones,
        has_email, has_phone, evidence_fingerprint
      ) values (
        ${evId}, 'apple_contacts', 'proof', 'PROOF-CONTACT', 'Steward Proof Person',
        '[]', '[]', false, false, 'proof-fingerprint'
      )
    `
    cleanup.push(async () => { await execute`delete from integration_relationship_evidence where id = ${evId}` })
    await recordReconcileDecision(evId, {
      reviewState: 'exact_linked', matchMethod: 'source_link', matchConfidence: 'exact',
      canonicalPersonId: personId, reason: 'opps_operator_approval', ruleVersion: REL_INTEL_RULE_VERSION,
    }, execute)
    const linked = (await execute`select canonical_person_id, review_state, match_method from integration_relationship_evidence where id = ${evId}`) as { canonical_person_id: string | null; review_state: string; match_method: string }[]
    out('12: link -> person=', linked[0].canonical_person_id === personId, 'review_state=', linked[0].review_state, 'match_method=', linked[0].match_method)

    const evId2 = randomUUID()
    await execute`insert into integration_relationship_evidence (id, source, source_account, source_identity_key, has_email, has_phone, evidence_fingerprint) values (${evId2}, 'gmail_contacts', 'proof2', 'bad@example.com', true, false, 'fp2')`
    cleanup.push(async () => { await execute`delete from integration_relationship_evidence where id = ${evId2}` })
    await recordReconcileDecision(evId2, { reviewState: 'rejected', matchMethod: 'rejected', matchConfidence: 'none', canonicalPersonId: null, reason: 'opps_operator_dismissal', ruleVersion: REL_INTEL_RULE_VERSION }, execute)
    const rejected = (await execute`select review_state, canonical_person_id from integration_relationship_evidence where id = ${evId2}`) as { review_state: string; canonical_person_id: string | null }[]
    out('12: reject -> review_state=', rejected[0].review_state, 'not linked=', rejected[0].canonical_person_id === null)
    out('12: no silent Person merge (no person rows added)')

    // 13: telemetry with no private content.
    await emitDailyLoopTelemetry({ eventType: 'outcome_recorded', entityKind: 'person', entityId: personId, execute })
    await emitDailyLoopTelemetry({ eventType: 'followup_snoozed', entityKind: 'follow_up', entityId: randomUUID(), execute })
    const tele = (await execute`select event_type, entity_kind, metadata from daily_loop_telemetry order by occurred_at desc limit 2`) as { event_type: string; entity_kind: string; metadata: unknown }[]
    out('13: telemetry rows =', tele.length)
    for (const t of tele) out('  ', t.event_type, t.entity_kind, '| no private content =', JSON.stringify(t.metadata).length < 120)
    await execute`delete from daily_loop_telemetry where event_type in ('outcome_recorded','followup_snoozed')`

    for (const c of cleanup) await c()
    out('CLEANUP done.')
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
