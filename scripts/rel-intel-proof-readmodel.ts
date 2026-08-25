// REL-INTEL — DEV read-model proof (Part 6) against real persistence.
// Temporarily links representative evidence (apple-only, gmail-only, both,
// bulk-heavy) to the real canonical persons, reads the per-Person read model,
// proves last-observed vs meaningful distinctions, then reverts. Run:
//   node --env-file=.env.local --import tsx scripts/rel-intel-proof-readmodel.ts
import {
  getRelationshipEvidenceForPerson,
  recordReconcileDecision,
} from '../db/relationship-evidence'
import { summarizeRelationshipEvidence } from '../lib/relationship-intel/relationship-context'
import { REL_INTEL_RULE_VERSION } from '../lib/relationship-intel/reconcile'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const persons = (await execute`select id, display_name from person where archived_at is null order by display_name`) as { id: string; display_name: string }[]
    const apple = (await execute`select id, source, source_identity_key from integration_relationship_evidence where source='apple_contacts' and review_state='unmatched' order by created_at limit 2`) as { id: string; source: string; source_identity_key: string }[]
    const gmailMeaningful = (await execute`select id, source, source_identity_key from integration_relationship_evidence where source='gmail_contacts' and review_state='review_required' order by created_at limit 2`) as { id: string; source: string; source_identity_key: string }[]
    const gmailBulk = (await execute`select id, source, source_identity_key from integration_relationship_evidence where source='gmail_contacts' and review_state='rejected' order by created_at limit 2`) as { id: string; source: string; source_identity_key: string }[]

    const interactions = (await execute`select person_id, count(*)::int as n from interaction group by person_id`) as { person_id: string; n: number }[]
    const manualByPerson = new Map(interactions.map((i) => [i.person_id, i.n]))

    if (persons.length < 4 || apple.length < 2 || gmailMeaningful.length < 2 || gmailBulk.length < 1) {
      console.log('insufficient DEV data for the representative matrix; got persons', persons.length, 'apple', apple.length, 'gmailMeaningful', gmailMeaningful.length, 'gmailBulk', gmailBulk.length)
      return
    }

    const linked: { evId: string; personId: string }[] = []
    const link = async (evId: string, personId: string) => {
      await recordReconcileDecision(
        evId,
        { reviewState: 'exact_linked', matchMethod: 'source_link', matchConfidence: 'exact', canonicalPersonId: personId, reason: 'readmodel_demonstration', ruleVersion: REL_INTEL_RULE_VERSION },
        execute,
      )
      linked.push({ evId, personId })
    }

    // apple-only person
    await link(apple[0].id, persons[0].id)
    // gmail-only person
    await link(gmailMeaningful[0].id, persons[1].id)
    // both apple + gmail on one person
    await link(apple[1].id, persons[2].id)
    await link(gmailMeaningful[1].id, persons[2].id)
    // bulk-heavy person (bulk + a meaningful row) -> meaningful must not refresh
    await link(gmailBulk[0].id, persons[3].id)
    await link(gmailMeaningful[0].id, persons[3].id)

    for (const p of persons) {
      const rows = await getRelationshipEvidenceForPerson(p.id, execute)
      const summary = summarizeRelationshipEvidence(rows)
      const manual = manualByPerson.get(p.id) ?? 0
      console.log('--- person', p.display_name, '| sources', JSON.stringify(summary.sources), '| manual interactions', manual)
      console.log('   hasEvidence:', summary.hasEvidence, '| lastObserved:', summary.lastObservedAt, '| lastMeaningful:', summary.lastMeaningfulContactAt, '| twoWay:', summary.twoWay)
      console.log('   lastInbound:', summary.lastInboundAt, '| lastOutbound:', summary.lastOutboundAt, '| coverageLimited:', summary.coverageLimited, '| reason:', summary.reason)
    }

    // Revert everything.
    for (const { evId } of linked) {
      await recordReconcileDecision(
        evId,
        { reviewState: 'unmatched', matchMethod: 'unmatched', matchConfidence: 'none', canonicalPersonId: null, reason: 'no_exact_match', ruleVersion: REL_INTEL_RULE_VERSION },
        execute,
      )
    }
    console.log('REVERTED', linked.length, 'temporary links.')
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
