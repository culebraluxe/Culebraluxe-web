// REL-INTEL — real bounded Gmail census load through the neutral ODS seam.
// Reads the approved bounded artifact, parses with the pure gmail-census parser,
// creates a bounded intake batch, persists accepted rows with the real
// upsertRelationshipEvidence seam, reconciles with the real reconcileEvidence
// engine (in-memory lookup), and reports exact batch-balance + reconciliation
// totals. Replay-safe. Run against DEV:
//   node --env-file=.env.local --import tsx scripts/rel-intel-load-gmail.ts
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { parseGmailCensus } from '../lib/relationship-intel/gmail-census'
import { reconcileEvidence } from '../lib/relationship-intel/reconcile'
import { createIntakeBatch, upsertRelationshipEvidence, getRelationshipEvidenceRows, recordReconcileDecision } from '../db/relationship-evidence'
import type { ReviewState } from '../lib/relationship-intel/contracts'
import { createPoolExecutor } from './lib/pool-executor'
import { createInMemoryPersonLookup, mapLimit } from '../lib/relationship-intel/inmemory-lookup'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''
const CONCURRENCY = 24
const CSV = 'docs/marlowe-gmail-relationship-census-private-2026-08-24.csv'
const SOURCE = 'gmail_contacts'
const EXTERNAL_BATCH_ID = 'marlowe-gmail-census-2026-08-24'
const OUTCOMES: ReviewState[] = ['exact_linked', 'review_required', 'ambiguous', 'unmatched', 'rejected', 'non_person', 'deferred', 'unresolved']

async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const csv = readFileSync(CSV, 'utf8')
    const balance = parseGmailCensus(csv)
    const declared = balance.declared
    const balanced = balance.accepted + balance.rejected + balance.quarantined
    console.log('GMAIL body rows (declared):', declared)
    console.log('GMAIL balance: declared=' + declared + ' accepted=' + balance.accepted +
      ' rejected=' + balance.rejected + ' quarantined=' + balance.quarantined +
      ' deduplicated=' + balance.deduplicated)
    console.log('GMAIL balance invariant declared==accepted+rejected+quarantined:', balanced === declared)

    const batchId = await createIntakeBatch(
      {
        source: SOURCE,
        sourceAccount: balance.rows[0]?.evidence.sourceAccount ?? 'gmail',
        externalBatchId: EXTERNAL_BATCH_ID,
        schemaVersion: 1,
        exportedAt: '2026-08-24',
        fileSha256: createHash('sha256').update(csv).digest('hex'),
        inputCount: balance.declared,
        validCount: balance.accepted,
        newProfileCount: balance.accepted,
        replayCount: 0,
        changedRevisionCount: 0,
        errorCount: balance.rejected + balance.quarantined,
      },
      execute,
    )
    console.log('GMAIL intake batch id:', batchId)

    const before = await getRelationshipEvidenceRows(SOURCE, execute)
    console.log('GMAIL evidence rows before load:', before.length)

    await mapLimit(balance.rows, CONCURRENCY, (row, i) =>
      upsertRelationshipEvidence(row.evidence, row.fingerprint, batchId, execute),
    )

    const rows = await getRelationshipEvidenceRows(SOURCE, execute)
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

    const after = await getRelationshipEvidenceRows(SOURCE, execute)
    console.log('GMAIL evidence rows after reconcile:', after.length)
    console.log('GMAIL new evidence rows (after - before):', after.length - before.length)
    console.log('GMAIL reconciliation totals:', JSON.stringify(tally))
    console.log('GMAIL canonical exact_linked with person:', canonicalLinked)

    const cov = await execute`
      select count(*)::int as n,
             min(first_observed_at) as first_min,
             max(last_observed_at) as last_max
      from integration_relationship_evidence where source = ${SOURCE}
    `
    console.log('GMAIL persisted coverage:', JSON.stringify(cov[0]))
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
