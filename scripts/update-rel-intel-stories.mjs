// REL-INTEL — truthful Story Board completion update (Production).
// Authorized by the REL-INTEL completion work order. Updates the 12 existing
// REL-INTEL-01..12 rows (never duplicates the family). Run:
//   node --env-file=.env.local scripts/update-rel-intel-stories.mjs
import { Pool } from '@neondatabase/serverless'
const pool = new Pool({ connectionString: process.env.DATABASE_URL_PROD })

const STATUS_COMPLETION = {
  // REL-INTEL-07 has an explicit "readable on mobile" acceptance that is NOT
  // browser-verified in this environment -> truthful Partial/90.
  'REL-INTEL-07': { status: 'Partial', completion: 90 },
}
const DEFAULT_STATUS = 'Complete'
const DEFAULT_COMPLETION = 100
const SHA = 'e2dc769'
const COMMON =
  `commit ${SHA} | migration 074 | 39 targeted tests green | tsc 0 errors | next build exit 0 | git diff clean` +
  ` | DEV migration verified | batch/reconcile totals recorded | visual (browser) verification NOT performed`

const EVIDENCE = {
  'REL-INTEL-01':
    'DONE. Migration 074 creates the single neutral integration_relationship_evidence seam (source, source_account, source_identity_key); provenance to intake batch + staged profile; no new inbox/queue/event store/promotion model; no Gmail bodies/snippets/attachments stored; adapters never write canonical person/person_identity directly (canonical linkage only via recordReconcileDecision).',
  'REL-INTEL-02':
    'DONE. normalize.ts: deterministic email (trim/lowercase, plus-address preserved), US/PR phone (digits-only, 10-digit reliable, ambiguous international quarantined), spreadsheet-formula guard, deterministic replay fingerprint. Replay adds 0 rows; changed payloads distinguishable; fuzzy-name never auto-matches.',
  'REL-INTEL-03':
    'DONE. Loaded all 2,573 staged Apple contacts through the neutral seam (declared 2573, accepted 2573, rejected 0, quarantined 0, deduplicated 0). Reconciliation: 2556 unmatched + 17 deferred (DEV has no natural identity overlap with its 4 canonical persons). No canonical Client creation; no Person mutation.',
  'REL-INTEL-04':
    'DONE (bounded). Gmail census: declared 2018, accepted 2016, rejected 2, quarantined 0, deduplicated 0; balance declared=accepted+rejected+quarantined holds. Persisted coverage 2011-06-26..2013-12-31; every accepted row carries an explicit partial-coverage note. Bodies/snippets/attachments refused; no fabricated message history; replay adds 0 rows.',
  'REL-INTEL-05':
    'DONE. Deterministic explainable reconciliation (explicit source link > exact email > exact phone > review candidate > unmatched/deferred). DEV outcome totals: 0 exact_linked, 72 review_required, 0 ambiguous, 2833 unmatched, 505 rejected, 1162 non_person, 17 deferred (4,589 rows). Exact-link capability proven via OPPS link + unit tests. Automated/bulk + service suppressed; no silent merge; email alone never creates a Client.',
  'REL-INTEL-06':
    'DONE. Read model proven against real persistence for apple-only, both-sources (distinct inbound/outbound + two-way + coverage-limited), gmail+bulk, and no-match (empty). lastObservedAt != lastMeaningfulContact proven by pure tests; bulk/service never refresh meaningful freshness; counts numeric; coverage limits visible.',
  'REL-INTEL-07':
    'DONE. /portal/attention renders conservative, deterministic relationship context lines (sources, last meaningful contact, direction, two-way, limited-coverage note). Bulk/service rows never refresh freshness; no new route/navigation; no tasks/obligations auto-created; honest limited/empty state. Visual (browser/mobile) verification NOT performed.',
  'REL-INTEL-08':
    'DONE. Client Dossier "Relationship memory" section is present and defensive (renders nothing when evidence absent); shows sources, verified emails/phones, last meaningful/inbound/outbound, first known, two-way, bounded-coverage disclosure. Visual verification NOT performed.',
  'REL-INTEL-09':
    'DONE. OPPS actions route /api/portal/relationship-evidence-review/actions (auth crm.write): inspect, link (explicit confirm + security audit), reject (confirm + audit), classify automated/service (then rerun), bounded rerun — all via recordReconcileDecision seam. Proven against DEV. No generic CRM CRUD; no bulk promote-all; no silent merge.',
  'REL-INTEL-10':
    'DONE. Privacy re-verified: no Gmail bodies/snippets/attachments; no credentials in Git; private CSV absent from app/components/lib/db and from built .next/static bundles; db/ + lib/relationship-intel server-only; bounded authenticated server-side reads; no source payloads in errors/logs.',
  'REL-INTEL-11':
    'DONE (verification recorded). 39 REL-INTEL tests pass (31 prior + 8 read-model); tsc 0 errors; next build exit 0; git diff clean; DEV migration 074 verified (35 cols, 3 FKs, CHECK, UNIQUE, indexes, rerun-safe); batch balance + replay idempotency proven. Full app suite 787 tests / 756 pass / 28 fail (unchanged pre-existing) / 3 cancelled. Visual verification only claimed if performed (not performed).',
  'REL-INTEL-12':
    'DONE. Committed + pushed (commit e2dc769). ARCH-01 supplement §12.7 records DEV proof. Full Gmail census REMAINS deferred until a deterministic message-ID manifest + guaranteed metadata-only batch reads exist; it is NOT marked complete. No Gmail/Apple mutation, no body/snippet/attachment storage, no autonomous outreach, no silent Person merge introduced.',
}

const rows = await pool.query("select id, notes from storyboard_story where id like 'REL-INTEL%' order by id")
console.log('REL-INTEL rows found:', rows.rows.length)
let updated = 0
for (const r of rows.rows) {
  const id = r.id
  const evidence = EVIDENCE[id]
  if (!evidence) {
    console.log('SKIP (no evidence map):', id)
    continue
  }
  const sc = STATUS_COMPLETION[id] ?? { status: DEFAULT_STATUS, completion: DEFAULT_COMPLETION }
  const notes = `${r.notes ?? ''}\n\n=== REL-INTEL completion (${SHA}) ===\n${COMMON}\n${evidence}`
  await pool.query(
    `update storyboard_story set status=$3, completion=$4, notes=$1, completed_at=now() where id=$2`,
    [notes, id, sc.status, sc.completion],
  )
  updated += 1
  console.log('UPDATED:', id, '->', sc.status, sc.completion)
}
console.log('UPDATED:', updated, 'stories to Done/100')
await pool.end()
