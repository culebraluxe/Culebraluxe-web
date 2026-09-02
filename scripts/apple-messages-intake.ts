// ---------------------------------------------------------------------------
// REL-INTEL — DURABLE Apple Messages intake / materialization command.
//
// The reusable operator/runtime entry point for the full downstream lifecycle
// once a valid Apple export package exists:
//
//   local Apple chat.db
//     -> export package (identities.jsonl + messages.jsonl + conversations)
//     -> ODS relationship-evidence upsert
//     -> reconciliation to canonical Person
//     -> canonical interaction materialization (exact_linked only)
//     -> client read-model refresh
//
// This is the PRODUCTION materialization process, NOT the DEV ODS proof loader
// (scripts/apple-messages-real-load.ts). It targets an EXPLICIT environment and
// refuses to fall back to the wrong DATABASE_URL.
//
// Usage (from a Terminal with Full Disk Access only needed for the *snapshot
// repair* step, not for this command):
//
//   DEV:  node --env-file=.env.local --import tsx scripts/apple-messages-intake.ts dev [--dir <path>]
//   PROD: DATABASE_URL_PROD=postgres://... node --env-file=.env.local \
//           --import tsx scripts/apple-messages-intake.ts prod [--dir <path>] [--evidence-only]
//
// Replay-safe: every canonical interaction reuses the createInteraction seam,
// backed by the unique partial index on (source_system, source_external_id).
// Running the same intake twice inserts N then 0 new interactions.
// ---------------------------------------------------------------------------
import { readFileSync, existsSync } from 'node:fs'
import {
  buildMessagesRelationshipEvidence,
  APPLE_MESSAGES_SOURCE,
  isGroupChatGuid,
  type AppleMessagesExport,
  type AppleMessagesHandle,
  type AppleMessagesMessage,
} from '../lib/relationship-intel/apple-messages'
import { boundedPreview } from '../lib/relationship-intel/apple-message-materializer'
import {
  upsertRelationshipEvidence,
  recordReconcileDecision,
  getRelationshipEvidenceRows,
} from '../db/relationship-evidence'
import { reconcileEvidence } from '../lib/relationship-intel/reconcile'
import { createInMemoryPersonLookup, mapLimit } from '../lib/relationship-intel/inmemory-lookup'
import { materializeAppleMessages } from '../db/apple-message-materialization'
import type { QueryExecutor } from '../db/query-executor'
import { createPoolExecutor } from './lib/pool-executor'

const DEFAULT_DIR = 'public/upload/data/apple-messages-export'
const out = (...a: unknown[]) => console.log(...a)

// ---------------------------------------------------------------------------
// Argument parsing — explicit DEV/PROD target, NO silent DATABASE_URL fallback.
// ---------------------------------------------------------------------------
type EnvTarget = 'dev' | 'prod'

function parseArgs(argv: string[]) {
  const targetArg = argv[0] ?? ''
  const target: EnvTarget | null =
    targetArg === 'dev' || targetArg === 'prod' ? targetArg : null
  if (!target) {
    throw new Error(
      'Usage: node --env-file=.env.local --import tsx scripts/apple-messages-intake.ts <dev|prod> [--dir <path>] [--evidence-only]',
    )
  }
  let dir = DEFAULT_DIR
  const dirIdx = argv.indexOf('--dir')
  if (dirIdx !== -1 && argv[dirIdx + 1]) dir = argv[dirIdx + 1]
  const evidenceOnly = argv.includes('--evidence-only')
  return { target, dir, evidenceOnly }
}

function resolveDatabaseUrl(target: EnvTarget): string {
  if (target === 'prod') {
    const url = process.env.DATABASE_URL_PROD ?? ''
    if (!url) {
      throw new Error(
        'PROD target requires DATABASE_URL_PROD. Refusing to fall back to DEV/local.',
      )
    }
    return url
  }
  const url = process.env.DATABASE_URL_DEV ?? ''
  if (!url) {
    throw new Error('DEV target requires DATABASE_URL_DEV (or set it explicitly).')
  }
  return url
}

// ---------------------------------------------------------------------------
// Loaders
// ---------------------------------------------------------------------------

function readLines(dir: string, file: string): string[] {
  const raw = readFileSync(`${dir}/${file}`, 'utf8')
  return raw.split('\n').filter((l) => l.trim().length > 0)
}

function loadExportPackage(dir: string) {
  if (!existsSync(`${dir}/identities.jsonl`) || !existsSync(`${dir}/messages.jsonl`)) {
    throw new Error(`Export package incomplete at ${dir} (need identities.jsonl + messages.jsonl)`)
  }
  const manifestPath = `${dir}/manifest.json`
  const manifest = existsSync(manifestPath)
    ? (JSON.parse(readFileSync(manifestPath, 'utf8')) as Record<string, unknown>)
    : {}
  const handles = readLines(dir, 'identities.jsonl').map(
    (l) => JSON.parse(l) as AppleMessagesHandle,
  )
  const messages = readLines(dir, 'messages.jsonl').map(
    (l) => JSON.parse(l) as AppleMessagesMessage,
  )
  const conversationsPath = `${dir}/conversations.jsonl`
  const conversationParticipantsPath = `${dir}/conversation-participants.jsonl`
  const conversations = existsSync(conversationsPath)
    ? readLines(dir, 'conversations.jsonl').map((l) => JSON.parse(l) as Record<string, unknown>)
    : []
  const conversationParticipants = existsSync(conversationParticipantsPath)
    ? readLines(dir, 'conversation-participants.jsonl').map(
        (l) => JSON.parse(l) as Record<string, unknown>,
      )
    : []
  return { manifest, handles, messages, conversations, conversationParticipants }
}

/** Deterministic source account derived from the export messages. */
function deriveSourceAccount(messages: AppleMessagesMessage[]): string {
  for (const m of messages) {
    const acct = (m as unknown as { account?: string }).account
    if (acct && acct.includes('@')) return acct
  }
  return 'apple_messages_local'
}

// ---------------------------------------------------------------------------
// Core reusable lifecycle (also importable by future runtime tools).
// ---------------------------------------------------------------------------

export type AppleMessagesIntakeResult = {
  env: EnvTarget
  sourceAccount: string
  handles: number
  messages: number
  datedMessages: number
  messagesWithBoundedText: number
  groupChatMessages: number
  evidenceRows: number
  exactLinkedHandles: number
  unmatchedOrReviewHandles: number
  eventsSeen: number
  interactionsInserted: number
  interactionsReplayed: number
  skippedNoTimestamp: number
  skippedGroupChat: number
  errors: number
  reconcileTally: Record<string, number>
}

export async function runAppleMessagesIntake(
  env: EnvTarget,
  exportDir: string,
  execute: QueryExecutor,
  options: { refresh?: () => Promise<void>; evidenceOnly?: boolean } = {},
): Promise<AppleMessagesIntakeResult> {
  const { manifest, handles, messages, conversations, conversationParticipants } =
    loadExportPackage(exportDir)
  const sourceAccount = deriveSourceAccount(messages)
  const exportData: AppleMessagesExport = { sourceAccount, handles, messages }

  out('=== Apple Messages intake / materialization ===')
  out('env:', env)
  out('export dir:', exportDir)
  out('manifest generatedAt:', (manifest as { generatedAt?: string }).generatedAt)
  out('sourceAccount derived:', sourceAccount)
  out('handles loaded:', handles.length, '| messages loaded:', messages.length)
  out('conversations loaded:', conversations.length, '| conversation-participants:', conversationParticipants.length)

  const datedMessages = messages.filter((m) => m.dateISO).length
  const messagesWithBoundedText = messages.filter(
    (m) => boundedPreview(m.text) != null,
  ).length
  const groupChatMessages = messages.filter((m) => isGroupChatGuid(m.chatGuid)).length
  out('dated messages:', datedMessages, '| with bounded text:', messagesWithBoundedText, '| group-chat messages:', groupChatMessages)

  // --- 1. ODS evidence build + upsert ---
  const evidenceBuilds = buildMessagesRelationshipEvidence(exportData)
  out('built evidence rows:', evidenceBuilds.length)

  const { lookup } = await createInMemoryPersonLookup(execute)
  const reconcileTally: Record<string, number> = {}
  let reconciledHandles = 0
  await mapLimit(evidenceBuilds, 16, async ({ evidence, fingerprint }) => {
    const id = await upsertRelationshipEvidence(evidence, fingerprint, undefined, execute)
    const decision = await reconcileEvidence(evidence, lookup)
    await recordReconcileDecision(id, decision, execute)
    reconcileTally[decision.reviewState] = (reconcileTally[decision.reviewState] ?? 0) + 1
    reconciledHandles += 1
    if (reconciledHandles % 100 === 0 || reconciledHandles === evidenceBuilds.length) {
      out(`reconcile progress: ${reconciledHandles}/${evidenceBuilds.length} handles`)
    }
  })
  out('reconcile tally:', JSON.stringify(reconcileTally))

  const evidenceRows = await getRelationshipEvidenceRows(APPLE_MESSAGES_SOURCE, execute)
  const exactLinkedHandles = evidenceRows.filter(
    (r) => r.reviewState === 'exact_linked' && r.canonicalPersonId != null,
  ).length
  const unmatchedOrReviewHandles = evidenceRows.length - exactLinkedHandles
  out('evidence rows:', evidenceRows.length, '| exact-linked handles:', exactLinkedHandles, '| unmatched/review handles:', unmatchedOrReviewHandles)

  // --- 2. Canonical interaction materialization (exact_linked only) ---
  const refresh = options.refresh ?? (async () => {
    await execute`refresh materialized view concurrently mv_client_relationship_channels`
    await execute`refresh materialized view concurrently mv_client_directory`
    await execute`refresh materialized view concurrently mv_client_contact_history`
  })
  const materialized = options.evidenceOnly
    ? {
        eventsSeen: 0,
        inserted: 0,
        replayed: 0,
        skippedNoTimestamp: 0,
        skippedGroupChat: 0,
        errors: 0,
      }
    : await materializeAppleMessages(exportData, execute, {
        refresh,
        progressEvery: 100,
        onProgress: (progress) => {
          out(
            `message progress: ${progress.processed}/${progress.eventsSeen}`,
            `inserted=${progress.inserted}`,
            `replayed=${progress.replayed}`,
            `skipped=${progress.skippedNoTimestamp + progress.skippedGroupChat}`,
            `errors=${progress.errors}`,
          )
        },
      })
  if (options.evidenceOnly) {
    out('evidence-only repair: interaction replay skipped')
    out('refreshing relationship channels, directory, and contact history')
    await refresh()
    const verification = await execute`
      with expected as (
        select
          canonical_person_id as person_id,
          sum(coalesce(inbound_count, 0))::bigint as inbound_count,
          sum(coalesce(outbound_count, 0))::bigint as outbound_count
        from integration_relationship_evidence
        where source = 'apple_messages'
          and canonical_person_id is not null
          and review_state = 'exact_linked'
          and is_automated_or_bulk is not true
          and is_organization_or_service is not true
        group by canonical_person_id
        having sum(coalesce(inbound_count, 0) + coalesce(outbound_count, 0)) > 0
            or max(coalesce(last_observed_at, last_outbound_at, last_inbound_at)) is not null
      ), actual as (
        select person_id, inbound_count, outbound_count
        from mv_client_relationship_channels
        where source = 'apple_messages'
      )
      select count(*)::int as mismatch_count
      from expected e
      full join actual a using (person_id)
      where coalesce(e.inbound_count, -1) <> coalesce(a.inbound_count, -1)
         or coalesce(e.outbound_count, -1) <> coalesce(a.outbound_count, -1)
    `
    const mismatchCount = Number(verification[0]?.mismatch_count ?? -1)
    if (mismatchCount !== 0) {
      throw new Error(`Client relationship-channel verification failed: ${mismatchCount} mismatched Apple summaries`)
    }
    out('verification OK: Apple evidence matches Client relationship summaries')
  }
  out('materialization:', JSON.stringify(materialized))

  return {
    env,
    sourceAccount,
    handles: handles.length,
    messages: messages.length,
    datedMessages,
    messagesWithBoundedText,
    groupChatMessages,
    evidenceRows: evidenceRows.length,
    exactLinkedHandles,
    unmatchedOrReviewHandles,
    eventsSeen: materialized.eventsSeen,
    interactionsInserted: materialized.inserted,
    interactionsReplayed: materialized.replayed,
    skippedNoTimestamp: materialized.skippedNoTimestamp,
    skippedGroupChat: materialized.skippedGroupChat,
    errors: materialized.errors,
    reconcileTally,
  }
}



function printTally(r: AppleMessagesIntakeResult) {
  out('\n=== INTAKE TALLY ===')
  out('env:', r.env)
  out('sourceAccount:', r.sourceAccount)
  out('handles:', r.handles)
  out('messages:', r.messages)
  out('dated messages:', r.datedMessages)
  out('messages with bounded text:', r.messagesWithBoundedText)
  out('group-chat messages:', r.groupChatMessages)
  out('evidence rows:', r.evidenceRows)
  out('exact-linked handles:', r.exactLinkedHandles)
  out('unmatched/review handles:', r.unmatchedOrReviewHandles)
  out('events seen:', r.eventsSeen)
  out('interactions inserted:', r.interactionsInserted)
  out('interactions replayed:', r.interactionsReplayed)
  out('skipped no timestamp:', r.skippedNoTimestamp)
  out('skipped group chat:', r.skippedGroupChat)
  out('errors:', r.errors)
  out('reconcile tally:', JSON.stringify(r.reconcileTally))
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

async function main() {
  const { target, dir, evidenceOnly } = parseArgs(process.argv.slice(2))
  const url = resolveDatabaseUrl(target)
  const { execute, end } = createPoolExecutor(url)
  try {
    // A single operator run performs ONE intake pass. Replay/idempotency is
    // proven in the regression harness (not by re-running tens of thousands of
    // historical messages a second time on every PROD run).
    const result = await runAppleMessagesIntake(target, dir, execute, { evidenceOnly })
    printTally(result)
  } finally {
    await end()
  }
}

// Allow import of runAppleMessagesIntake without executing the CLI.
if (process.argv[1]?.endsWith('apple-messages-intake.ts')) {
  main()
    .then(() => process.exit(0))
    .catch((err) => {
      console.error(err)
      process.exit(1)
    })
}
