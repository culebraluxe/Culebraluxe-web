// REL-INTEL — DEV proof of the OPPS stewardship loop against real persistence.
// Exercises the exact repository functions the OPPS actions route calls:
//   link (exact_linked/source_link) -> rerun keeps exact -> read model reflects it
//   classify (automated/service)    -> rerun flips to rejected/non_person
// and then reverts so DEV totals stay clean. Run against DEV:
//   node --env-file=.env.local --import tsx scripts/rel-intel-proof-opps.ts
import {
  getRelationshipEvidenceForPerson,
  getRelationshipEvidenceById,
  recordReconcileDecision,
  classifyEvidenceRow,
  rerunRelationshipReconciliation,
} from '../db/relationship-evidence'
import { REL_INTEL_RULE_VERSION } from '../lib/relationship-intel/reconcile'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const persons = (await execute`select id, display_name from person where archived_at is null order by display_name limit 3`) as { id: string; display_name: string }[]
    const unmatched = (await execute`select id, source, source_identity_key from integration_relationship_evidence where review_state='unmatched' order by created_at limit 3`) as { id: string; source: string; source_identity_key: string }[]

    if (persons.length === 0 || unmatched.length < 2) {
      console.log('not enough persons/unmatched rows to demonstrate; skipping')
      return
    }

    // --- LINK ---
    const person = persons[0]
    const ev = unmatched[0]
    console.log('LINK: evidence', ev.id, 'source', ev.source, '-> person', person.id, person.display_name)
    await recordReconcileDecision(
      ev.id,
      { reviewState: 'exact_linked', matchMethod: 'source_link', matchConfidence: 'exact', canonicalPersonId: person.id, reason: 'opps_operator_approval', ruleVersion: REL_INTEL_RULE_VERSION },
      execute,
    )
    const linked = await getRelationshipEvidenceById(ev.id, execute)
    console.log('LINK after: review_state=', linked?.reviewState, 'match_method=', linked?.matchMethod, 'canonical_person_id=', linked?.canonicalPersonId)

    // --- READ MODEL ---
    const rm = await getRelationshipEvidenceForPerson(person.id, execute)
    console.log('READ MODEL for person has', rm.length, 'evidence row(s); contains linked row:', rm.some((r) => r.id === ev.id))

    // --- RERUN keeps explicit source link ---
    const rerun = await rerunRelationshipReconciliation({ ids: [ev.id], limit: 1 }, execute)
    console.log('RERUN after link tally:', JSON.stringify(rerun.tally), 'canonicalLinked:', rerun.canonicalLinked)

    // --- CLASSIFY automated -> rejected ---
    const ev2 = unmatched[1]
    await classifyEvidenceRow(ev2.id, { isAutomatedOrBulk: true }, execute)
    const rerun2 = await rerunRelationshipReconciliation({ ids: [ev2.id], limit: 1 }, execute)
    const row2 = await getRelationshipEvidenceById(ev2.id, execute)
    console.log('CLASSIFY automated -> review_state=', row2?.reviewState, 'reason=', row2?.matchReason, 'rerun tally=', JSON.stringify(rerun2.tally))

    // --- REVERT to keep DEV totals clean ---
    await recordReconcileDecision(
      ev.id,
      { reviewState: 'unmatched', matchMethod: 'unmatched', matchConfidence: 'none', canonicalPersonId: null, reason: 'no_exact_match', ruleVersion: REL_INTEL_RULE_VERSION },
      execute,
    )
    await classifyEvidenceRow(ev2.id, { isAutomatedOrBulk: null }, execute)
    await rerunRelationshipReconciliation({ ids: [ev.id, ev2.id], limit: 5 }, execute)
    console.log('REVERTED both rows to original state.')
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
