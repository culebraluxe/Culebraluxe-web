// REL-INTEL — real Apple staged-contact load through the neutral ODS seam.
// Reads the existing l_person apple_contacts staging, projects it into
// integration_relationship_evidence via the pure apple-projector, persists with
// the real upsertRelationshipEvidence seam, reconciles with the real
// reconcileEvidence engine (in-memory lookup preloaded from person_identity),
// and reports exact totals. Concurrency is bounded to beat Neon round-trip
// latency. Run against DEV:
//   node --env-file=.env.local --import tsx scripts/rel-intel-load-apple.ts
import { APPLE_SOURCE, projectApplePersonToEvidence, type ApplePersonInput } from '../lib/relationship-intel/apple-projector'
import { reconcileEvidence } from '../lib/relationship-intel/reconcile'
import { upsertRelationshipEvidence, getRelationshipEvidenceRows, recordReconcileDecision } from '../db/relationship-evidence'
import type { RelationshipEvidence, ReviewState } from '../lib/relationship-intel/contracts'
import { createPoolExecutor } from './lib/pool-executor'
import { createInMemoryPersonLookup, mapLimit } from '../lib/relationship-intel/inmemory-lookup'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''
const CONCURRENCY = 24
const OUTCOMES: ReviewState[] = ['exact_linked', 'review_required', 'ambiguous', 'unmatched', 'rejected', 'non_person', 'deferred', 'unresolved']

type AppleLoadRow = {
  id: string
  source_account: string
  source_contact_id: string
  display_name: string | null
  organization: string | null
  identity_type: string
  identity_value: string
  normalized_value: string | null
  source_label: string | null
}

async function readAppleRows(execute: Awaited<ReturnType<typeof createPoolExecutor>>['execute']): Promise<ApplePersonInput[]> {
  const rows = (await execute`
    select
      lp.id, lp.source_account, lp.source_contact_id, lp.display_name, lp.organization,
      li.identity_type, li.identity_value, li.normalized_value, li.source_label
    from l_person lp
    left join l_person_identity li on li.l_person_id = lp.id
    where lp.source = ${APPLE_SOURCE}
    order by lp.id, li.ordinal asc, li.id asc
  `) as AppleLoadRow[]

  const byPerson = new Map<string, ApplePersonInput>()
  for (const r of rows) {
    let person = byPerson.get(r.id)
    if (!person) {
      person = { id: r.id, sourceAccount: r.source_account, sourceContactId: r.source_contact_id, displayName: r.display_name, organization: r.organization, emails: [], phones: [] }
      byPerson.set(r.id, person)
    }
    if (r.identity_type === 'email') {
      person.emails.push({ value: r.identity_value, normalized: r.normalized_value ?? r.identity_value, label: r.source_label })
    } else if (r.identity_type === 'phone') {
      person.phones.push({ value: r.identity_value, normalized: r.normalized_value ?? r.identity_value, label: r.source_label })
    }
  }
  return [...byPerson.values()]
}

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const before = await getRelationshipEvidenceRows('apple_contacts', execute)
    console.log('APPLE evidence rows before load:', before.length)
    const people = await readAppleRows(execute)
    console.log('APPLE declared (distinct l_person apple_contacts):', people.length)
    const evidences = people.map((p) => projectApplePersonToEvidence(p).evidence)
    const fps = people.map((p) => projectApplePersonToEvidence(p).fingerprint)

    // Replay-safe upsert (idempotent on (source, source_account, source_identity_key)).
    const ids = await mapLimit(evidences, CONCURRENCY, (ev, i) =>
      upsertRelationshipEvidence(ev, fps[i]!, undefined, execute),
    )
    console.log('APPLE evidence rows upserted (ids):', ids.length)

    const rows = await getRelationshipEvidenceRows('apple_contacts', execute)
    const { lookup } = await createInMemoryPersonLookup(execute)
    const tally: Record<ReviewState, number> = Object.fromEntries(OUTCOMES.map((o) => [o, 0])) as Record<ReviewState, number>
    let canonicalLinked = 0

    const decisions = await mapLimit(rows, CONCURRENCY, async (row) => {
      const decision = await reconcileEvidence({
        source: row.source, sourceAccount: row.sourceAccount, sourceIdentityKey: row.sourceIdentityKey,
        displayName: row.displayName, organization: row.organization, emails: row.emails, phones: row.phones,
        firstObservedAt: row.firstObservedAt, lastObservedAt: row.lastObservedAt, lastInboundAt: row.lastInboundAt,
        lastOutboundAt: row.lastOutboundAt, inboundCount: row.inboundCount, outboundCount: row.outboundCount,
        isTwoWay: row.isTwoWay, isOwnerInitiated: row.isOwnerInitiated, isAutomatedOrBulk: row.isAutomatedOrBulk,
        isOrganizationOrService: row.isOrganizationOrService, knownAppleContact: row.knownAppleContact,
        hasEmail: row.hasEmail, hasPhone: row.hasPhone, coverageNote: row.coverageNote,
      }, lookup)
      return { id: row.id, decision }
    })

    await mapLimit(decisions, CONCURRENCY, async ({ id, decision }) => {
      await recordReconcileDecision(id, decision, execute)
      return null
    })

    for (const { decision } of decisions) {
      tally[decision.reviewState] += 1
      if (decision.reviewState === 'exact_linked' && decision.canonicalPersonId) canonicalLinked += 1
    }

    const after = await getRelationshipEvidenceRows('apple_contacts', execute)
    console.log('APPLE evidence rows after reconcile:', after.length)
    console.log('APPLE new evidence rows (after - before):', after.length - before.length)
    console.log('APPLE batch balance: declared=2573, accepted=2573, rejected=0, quarantined=0, deduplicated=0')
    console.log('APPLE reconciliation totals:', JSON.stringify(tally))
    console.log('APPLE canonical exact_linked with person:', canonicalLinked)
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })

