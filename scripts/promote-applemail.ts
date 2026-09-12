#!/usr/bin/env node
// ---------------------------------------------------------------------------
// THE MAIL PROMOTION — l_applemail -> Warehouse. The ONLY thing that reads an
// l_ table for mail. ODS is pure intake; nothing client-facing may read it.
//
//   l_applemail            source evidence, written by the intake
//     -> observations      neutral, one normalization implementation
//     -> integration_relationship_evidence  reconciled to a canonical Person
//     -> interaction       the canonical comms event the CRM pane reads
//     -> refresh mv_client_relationship_channels / mv_client_directory /
//                mv_client_contact_history
//
// Idempotent. Evidence upserts on its fingerprint, the reconcile decision is
// recorded on the evidence row, and interactions are replay-safe on
// (source_system, source_external_id), so a re-run lands nothing twice.
//
//   node --env-file=.env.local --import tsx scripts/promote-applemail.ts prod
//   node --env-file=.env.local --import tsx scripts/promote-applemail.ts prod --days 30 --verify
// ---------------------------------------------------------------------------
import { createPoolExecutor } from './lib/pool-executor'
import type { QueryExecutor } from '../db/query-executor'
import {
  getRelationshipEvidenceRows,
  recordReconcileDecision,
  upsertRelationshipEvidence,
} from '../db/relationship-evidence'
import { createInteraction } from '../db/interactions'
import { createInMemoryPersonLookup } from '../lib/relationship-intel/inmemory-lookup'
import { reconcileEvidence } from '../lib/relationship-intel/reconcile'
import {
  ICLOUD_MAIL_SOURCE,
  buildICloudMailEvidence,
  normalizeMailbox,
  observationToInteraction,
} from '../lib/relationship-intel/icloud-mail'
import { normalizeLandedMail, type LandedAppleMailRow } from '../lib/relationship-intel/applemail'

const argv = process.argv.slice(2)

function flag(name: string): string | undefined {
  const index = argv.indexOf(name)
  return index >= 0 && index + 1 < argv.length ? argv[index + 1] : undefined
}

function die(message: string): never {
  console.error(message)
  process.exit(2)
}

const target = argv.find((arg) => arg === 'dev' || arg === 'prod')
if (!target) {
  die(
    'Usage: promote-applemail.ts <dev|prod> [--days N] [--account email] [--verify]\n' +
      '  Reads l_applemail and promotes it into relationship evidence + interactions.',
  )
}

const daysRaw = flag('--days') ?? process.env.MAIL_PROMOTE_DAYS?.trim() ?? '90'
const days = Number(daysRaw)
if (!Number.isFinite(days) || days <= 0) die(`--days must be a positive number of days (got "${daysRaw}")`)

const onlyAccount = flag('--account')?.trim().toLowerCase() || null
const verifyOnly = argv.includes('--verify')

if (target === 'prod') {
  if (!process.env.DATABASE_URL_PROD) die('No DATABASE_URL_PROD configured (fail closed)')
  if (process.env.DATABASE_URL_PROD === process.env.DATABASE_URL_DEV) {
    die('PROD selected but the connection is the DEV URL (fail closed)')
  }
}
const url = target === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
if (!url) die(`No DATABASE_URL_${target.toUpperCase()} configured (fail closed)`)

/** Internal addresses define which side of a message is the counterparty. */
function internalAddresses(): Set<string> {
  const raw = (process.env.EMAIL_INTERNAL_ADDRESSES ?? '').trim()
  if (!raw) die('EMAIL_INTERNAL_ADDRESSES is required: it decides inbound vs outbound.')
  const normalized = raw
    .split(',')
    .map((entry) => normalizeMailbox(entry))
    .filter((value): value is string => Boolean(value))
  if (normalized.length === 0) die('EMAIL_INTERNAL_ADDRESSES lists no usable addresses.')
  return new Set(normalized)
}

async function readLandedMail(
  execute: QueryExecutor,
  windowDays: number,
  account: string | null,
): Promise<LandedAppleMailRow[]> {
  const rows = (await execute`
    select source_account, source_message_id, mailbox_kind, mailbox_name, local_id,
           message_id, occurred_at, sender, to_recipients, cc_recipients,
           bcc_recipients, subject
      from l_applemail
     where occurred_at >= now() - make_interval(days => ${windowDays})
       and (${account}::text is null or source_account = ${account})
     order by occurred_at asc
  `) as unknown as LandedAppleMailRow[]
  return rows
}

async function refreshClientReadModels(execute: QueryExecutor) {
  await execute`refresh materialized view concurrently mv_client_relationship_channels`
  await execute`refresh materialized view concurrently mv_client_directory`
  await execute`refresh materialized view concurrently mv_client_contact_history`
}

async function promote(execute: QueryExecutor, observations: ReturnType<typeof normalizeLandedMail>['observations']) {
  // Evidence, reconciled to a canonical Person, one row per counterparty.
  const builds = buildICloudMailEvidence(observations)
  const { lookup } = await createInMemoryPersonLookup(execute)
  const tally: Record<string, number> = {}
  for (const build of builds) {
    const id = await upsertRelationshipEvidence(build.evidence, build.fingerprint, undefined, execute)
    const decision = await reconcileEvidence(build.evidence, lookup)
    await recordReconcileDecision(id, decision, execute)
    tally[decision.reviewState] = (tally[decision.reviewState] ?? 0) + 1
  }
  console.log(`evidence: ${builds.length} counterparties, reconcile tally ${JSON.stringify(tally)}`)

  // Only evidence already linked to a Person can become a comms event.
  const syncedAccounts = new Set(observations.map((observation) => observation.sourceAccount))
  const linked = new Map(
    (await getRelationshipEvidenceRows(ICLOUD_MAIL_SOURCE, execute))
      .filter((row) => syncedAccounts.has(row.sourceAccount))
      .filter((row) => row.reviewState === 'exact_linked' && row.canonicalPersonId)
      .map((row) => [row.sourceIdentityKey, row.canonicalPersonId!]),
  )

  let inserted = 0
  let replayed = 0
  let unlinked = 0
  for (const observation of observations) {
    const personId = linked.get(observation.externalEmail)
    if (!personId) {
      unlinked += 1
      continue
    }
    const result = await createInteraction(observationToInteraction(observation, personId), execute)
    if (result.created) inserted += 1
    else replayed += 1
  }

  await refreshClientReadModels(execute)
  console.log(
    `promotion complete: interactions inserted=${inserted} replayed=${replayed} ` +
      `unlinked=${unlinked} (evidence stays staged for the unlinked)`,
  )
}

async function main() {
  const pool = createPoolExecutor(url!)
  try {
    const execute = pool.execute
    const internal = internalAddresses()

    const landed = await readLandedMail(execute, days, onlyAccount)
    const { observations, skipped } = normalizeLandedMail(landed, internal)
    console.log(
      `l_applemail: ${landed.length} landed rows in the last ${days} day(s)` +
        `${onlyAccount ? ` for ${onlyAccount}` : ''} -> ${observations.length} observations`,
    )
    console.log(`skipped: ${JSON.stringify(skipped)}`)

    if (verifyOnly) {
      const accounts = [...new Set(observations.map((observation) => observation.sourceAccount))]
      console.log(`verify only — no writes. accounts=${accounts.join(', ') || '(none)'}`)
      return
    }
    if (observations.length === 0) {
      console.log('nothing to promote.')
      return
    }

    await promote(execute, observations)
  } finally {
    await pool.end()
  }
}

main().catch((error) => {
  console.error(`Apple Mail promotion failed: ${error instanceof Error ? error.message : String(error)}`)
  process.exitCode = 1
})
