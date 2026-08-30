#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Apple Contacts — canonical promotion stage (PROD). Closes the lifecycle:
//   l_person (projected) -> relationship evidence -> reconcile -> promote
//   unmatched/review_required apple evidence -> canonical Person -> MV refresh.
//
// PROD only. Fail-closed. Idempotent / replay-safe:
//   - evidence upsert is idempotent on (source, source_account, source_identity_key)
//   - reconciliation records deterministic decisions on each evidence row
//   - promotion dedupes by primary identity, reuses existing Persons, and links
//     evidence; re-running finds the just-created identity and links existing
//     (0 duplicate Person / identity).
//
// The client read models (mv_client_directory / mv_client_contact_history) are
// refreshed ONCE per promotion cycle via the shared seam.
//
//   APP_ENV=production node --env-file=.env.local --import tsx scripts/promote-apple-contacts.ts --env prod
// ---------------------------------------------------------------------------
import { sql } from '../db/client'
import { loadAppleEvidence } from '../lib/relationship-intel/apple-evidence'
import { reconcileEvidence } from '../lib/relationship-intel/reconcile'
import { createInMemoryPersonLookup, mapLimit } from '../lib/relationship-intel/inmemory-lookup'
import {
  getRelationshipEvidenceRows,
  recordReconcileDecision,
  type RelationshipEvidenceRow,
} from '../db/relationship-evidence'
import { promoteEvidence } from '../db/promote-evidence'
import { APPLE_SOURCE } from '../lib/relationship-intel/apple-projector'
import type {
  RelationshipEvidence,
  ReviewState,
} from '../lib/relationship-intel/contracts'

const CONCURRENCY = 24
const OUTCOMES: ReviewState[] = [
  'exact_linked', 'review_required', 'ambiguous', 'unmatched',
  'rejected', 'non_person', 'deferred', 'unresolved',
]
const JESSICA_EMAIL = 'jessica@bodysoulandbeauty.com'

function flag(name: string): string | undefined {
  const i = process.argv.indexOf(name)
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : undefined
}

/** Rebuild a plain RelationshipEvidence from a persisted row (re-reconcile). */
function toEvidence(row: RelationshipEvidenceRow): RelationshipEvidence {
  return {
    source: row.source,
    sourceAccount: row.sourceAccount,
    sourceIdentityKey: row.sourceIdentityKey,
    sourceLabel: row.sourceLabel,
    displayName: row.displayName,
    organization: row.organization,
    emails: row.emails,
    phones: row.phones,
    firstObservedAt: row.firstObservedAt,
    lastObservedAt: row.lastObservedAt,
    lastInboundAt: row.lastInboundAt,
    lastOutboundAt: row.lastOutboundAt,
    inboundCount: row.inboundCount,
    outboundCount: row.outboundCount,
    isTwoWay: row.isTwoWay,
    isOwnerInitiated: row.isOwnerInitiated,
    isAutomatedOrBulk: row.isAutomatedOrBulk,
    isOrganizationOrService: row.isOrganizationOrService,
    knownAppleContact: row.knownAppleContact,
    hasEmail: row.hasEmail,
    hasPhone: row.hasPhone,
    coverageNote: row.coverageNote,
  }
}

async function runMain(): Promise<void> {
  const env = (flag('--env') ?? 'dev').toLowerCase()
  if (env !== 'prod') {
    console.error('promote-apple-contacts is PROD-only (fail closed)')
    process.exit(2)
  }
  if (!process.env.DATABASE_URL_PROD) {
    console.error('No DATABASE_URL_PROD configured (fail closed)')
    process.exit(2)
  }
  if (process.env.DATABASE_URL_PROD === process.env.DATABASE_URL_DEV) {
    console.error('PROD promotion selected but the configured connection is the DEV URL (fail closed)')
    process.exit(2)
  }
  // Route the shared db/client gateway to PROD for this operator run.
  process.env.APP_ENV = 'production'

  // 1. Relationship evidence (replay-safe upsert from the projected l_person rows).
  const projected = await loadAppleEvidence(sql)
  console.log(`[contacts-sync] apple evidence projected: ${projected} contacts`)

  // 2. Reconcile all apple evidence (exact match / ambiguous / unmatched / …).
  const rows = await getRelationshipEvidenceRows(APPLE_SOURCE, sql)
  const { lookup } = await createInMemoryPersonLookup(sql)
  const tally: Record<ReviewState, number> = Object.fromEntries(
    OUTCOMES.map((o) => [o, 0]),
  ) as Record<ReviewState, number>
  let exactLinked = 0

  const decisions = await mapLimit(rows, CONCURRENCY, async (row) => {
    const decision = await reconcileEvidence(toEvidence(row), lookup)
    return { id: row.id, decision }
  })
  await mapLimit(decisions, CONCURRENCY, async ({ id, decision }) => {
    await recordReconcileDecision(id, decision, sql)
    return null
  })
  for (const { decision } of decisions) {
    tally[decision.reviewState] += 1
    if (decision.reviewState === 'exact_linked' && decision.canonicalPersonId) exactLinked += 1
  }
  console.log(`[contacts-sync] reconciled ${rows.length} apple contacts; exact_linked=${exactLinked} ${JSON.stringify(tally)}`)

  // 3. Promote safe unmatched + review-required apple evidence -> canonical Person
  //    (deduped by identity; reuses existing; links evidence). Refreshes the MV once.
  const peopleBefore = Number(((await sql`select count(*)::int as n from person`) as { n: number }[])[0]?.n ?? 0)
  const result = await promoteEvidence({
    source: APPLE_SOURCE,
    reviewStates: ['review_required', 'unmatched'],
  })
  const peopleAfter = Number(((await sql`select count(*)::int as n from person`) as { n: number }[])[0]?.n ?? 0)

  const byState = (await sql`
    select review_state, count(*)::int as n
    from integration_relationship_evidence
    where source = ${APPLE_SOURCE}
    group by review_state order by review_state
  `) as { review_state: string; n: number }[]
  const reviewRemaining = byState
    .filter((r) => ['review_required', 'unmatched', 'ambiguous'].includes(r.review_state))
    .reduce((s, r) => s + Number(r.n), 0)
  const skipped = byState
    .filter((r) => ['non_person', 'rejected', 'deferred'].includes(r.review_state))
    .reduce((s, r) => s + Number(r.n), 0)

  console.log(
    `[contacts-sync] promotion complete: created=${result.created} linked=${result.linkedExisting} ` +
      `enriched=${result.enriched} identitiesAdded=${result.identitiesAdded} conflicts=${result.conflicts} ` +
      `review=${reviewRemaining} skipped=${skipped} (person rows ${peopleBefore} -> ${peopleAfter})`,
  )
  console.log('[contacts-sync] client read model refreshed (mv_client_directory + mv_client_contact_history)')

  // 4. Jessica acceptance proof (the real-world case).
  const jessicaPeople = (await sql`
    select id, display_name from person
    where display_name ilike '%jessica%' order by display_name
  `) as { id: string; display_name: string }[]
  const jessicaRow = (await sql`
    select person_id from person_identity
    where identity_type = 'email' and identity_value = ${JESSICA_EMAIL}
    limit 1
  `) as { person_id: string }[]
  const jessicaPersonId = jessicaRow[0]?.person_id ?? null
  const jessicaIdentities = jessicaPersonId
    ? ((await sql`
        select identity_type, identity_value from person_identity
        where person_id = ${jessicaPersonId} order by identity_type, identity_value
      `) as { identity_type: string; identity_value: string }[])
    : []
  const jessicaInDirectory = jessicaPersonId
    ? (await sql`
        select person_id, display_name from mv_client_directory
        where person_id = ${jessicaPersonId} limit 1
      `) as { person_id: string; display_name: string }[]
    : []

  console.log(JSON.stringify({
    env,
    source: APPLE_SOURCE,
    stage: 'promotion',
    totals: {
      evidenceProjected: projected,
      reconciled: rows.length,
      exactLinked,
      created: result.created,
      linkedExisting: result.linkedExisting,
      enriched: result.enriched,
      identitiesAdded: result.identitiesAdded,
      conflicts: result.conflicts,
      evidenceLinked: result.evidenceLinked,
      peopleBefore,
      peopleAfter,
      reviewRemaining,
      skipped,
    },
    jessica: {
      email: JESSICA_EMAIL,
      matchingDisplayNamePeople: jessicaPeople.length,
      canonicalPersonId: jessicaPersonId,
      identities: jessicaIdentities,
      inClientDirectory: jessicaInDirectory.length > 0,
    },
  }, null, 2))
}

runMain()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error('[contacts-sync] promotion failed:', (err as Error).message ?? String(err))
    process.exit(1)
  })

