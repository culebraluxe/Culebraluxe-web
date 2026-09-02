import { existsSync, readFileSync } from 'node:fs'

import {
  APPLE_CALLS_SOURCE,
  APPLE_FACETIME_SOURCE,
  buildAppleCallRelationshipEvidence,
  callDirection,
  callSource,
  type AppleCallRecord,
} from '../lib/relationship-intel/apple-calls'
import { upsertRelationshipEvidence, recordReconcileDecision, getRelationshipEvidenceRows } from '../db/relationship-evidence'
import { reconcileEvidence } from '../lib/relationship-intel/reconcile'
import { createInMemoryPersonLookup, mapLimit } from '../lib/relationship-intel/inmemory-lookup'
import { createInteraction } from '../db/interactions'
import type { QueryExecutor } from '../db/query-executor'
import { createPoolExecutor } from './lib/pool-executor'

const DEFAULT_FILE = 'public/upload/data/apple-messages-export/calls.jsonl'

type EnvTarget = 'dev' | 'prod'

function parseArgs(argv: string[]) {
  const target = argv[0] === 'dev' || argv[0] === 'prod' ? argv[0] as EnvTarget : null
  if (!target) throw new Error('Usage: apple-calls-intake.ts <dev|prod> [--file <calls.jsonl>]')
  const i = argv.indexOf('--file')
  return { target, file: i >= 0 && argv[i + 1] ? argv[i + 1] : DEFAULT_FILE }
}

function databaseUrl(target: EnvTarget): string {
  const value = target === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  if (!value) throw new Error(`${target.toUpperCase()} database URL is required; refusing fallback.`)
  return value
}

function loadCalls(file: string): AppleCallRecord[] {
  if (!existsSync(file)) throw new Error(`CallHistory export missing: ${file}`)
  return readFileSync(file, 'utf8')
    .split('\n')
    .filter((line) => line.trim())
    .map((line) => JSON.parse(line) as AppleCallRecord)
}

async function refresh(execute: QueryExecutor) {
  await execute`refresh materialized view concurrently mv_client_relationship_channels`
  await execute`refresh materialized view concurrently mv_client_directory`
  await execute`refresh materialized view concurrently mv_client_contact_history`
}

async function run(target: EnvTarget, file: string, execute: QueryExecutor) {
  const calls = loadCalls(file)
  const sourceAccount = 'apple_call_history_local'
  const evidenceBuilds = buildAppleCallRelationshipEvidence(calls, sourceAccount)
  const { lookup } = await createInMemoryPersonLookup(execute)
  const tally: Record<string, number> = {}

  await mapLimit(evidenceBuilds, 16, async ({ evidence, fingerprint }) => {
    const id = await upsertRelationshipEvidence(evidence, fingerprint, undefined, execute)
    const decision = await reconcileEvidence(evidence, lookup)
    await recordReconcileDecision(id, decision, execute)
    tally[decision.reviewState] = (tally[decision.reviewState] ?? 0) + 1
  })

  const linkedBySource = new Map<string, Map<string, string>>()
  for (const source of [APPLE_CALLS_SOURCE, APPLE_FACETIME_SOURCE] as const) {
    const rows = await getRelationshipEvidenceRows(source, execute)
    const byAddress = new Map<string, string>()
    for (const row of rows) {
      if (row.reviewState === 'exact_linked' && row.canonicalPersonId) {
        byAddress.set(row.sourceIdentityKey, row.canonicalPersonId)
      }
    }
    linkedBySource.set(source, byAddress)
  }

  let inserted = 0
  let replayed = 0
  let skippedUnlinked = 0
  let skippedNoDate = 0
  let skippedNoAddress = 0
  let errors = 0

  for (const call of calls) {
    const address = call.address?.trim()
    if (!address) { skippedNoAddress += 1; continue }
    if (!call.dateISO) { skippedNoDate += 1; continue }
    const source = callSource(call)
    const personId = linkedBySource.get(source)?.get(address)
    if (!personId) { skippedUnlinked += 1; continue }

    try {
      const duration = call.duration == null ? undefined : Math.max(0, Math.round(Number(call.duration)))
      const eventType = source === APPLE_FACETIME_SOURCE ? 'facetime_call' : 'phone_call'
      const result = await createInteraction({
        personId,
        channel: 'call',
        eventType,
        direction: callDirection(call),
        occurredAt: call.dateISO,
        durationSeconds: Number.isFinite(duration) ? duration : undefined,
        sourceSystem: source,
        sourceExternalId: call.uniqueId || `row:${call.rowid}`,
        sourceMetadata: {
          address,
          answered: call.answered == null ? null : Boolean(call.answered),
          callType: call.callType == null ? null : String(call.callType),
          serviceProvider: call.serviceProvider ?? null,
          countryCode: call.countryCode ?? null,
        },
      }, execute)
      if (result.created) inserted += 1
      else replayed += 1
    } catch {
      errors += 1
    }
  }

  if (evidenceBuilds.length > 0 || inserted > 0) await refresh(execute)

  console.log('=== APPLE CALL HISTORY INTAKE ===')
  console.log('env:', target)
  console.log('calls:', calls.length)
  console.log('normal calls:', calls.filter((c) => callSource(c) === APPLE_CALLS_SOURCE).length)
  console.log('facetime calls:', calls.filter((c) => callSource(c) === APPLE_FACETIME_SOURCE).length)
  console.log('evidence rows built:', evidenceBuilds.length)
  console.log('reconcile tally:', JSON.stringify(tally))
  console.log('interactions inserted:', inserted)
  console.log('interactions replayed:', replayed)
  console.log('skipped unlinked:', skippedUnlinked)
  console.log('skipped no date:', skippedNoDate)
  console.log('skipped no address:', skippedNoAddress)
  console.log('errors:', errors)
}

async function main() {
  const { target, file } = parseArgs(process.argv.slice(2))
  const { execute, end } = createPoolExecutor(databaseUrl(target))
  try { await run(target, file, execute) } finally { await end() }
}

main().catch((error) => { console.error(error); process.exit(1) })
