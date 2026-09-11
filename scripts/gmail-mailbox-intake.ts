// ---------------------------------------------------------------------------
// GMAIL MAILBOX INTAKE — bounded, resumable, replay-safe.
//
// RESTORES the real Gmail mailbox census. This is NOT the newest-context feature
// (scripts/gmail-metadata-sync.ts), and it does NOT begin from CRM identities:
// source evidence lands FIRST, regardless of whether the correspondent already
// exists in CRM.
//
//   source -> one date shard (a calendar month)
//          -> bounded page (maxResults=500 via users.messages.list)
//          -> bounded metadata gets (format=metadata, 10 at a time)
//          -> landEmail immediately, per record
//          -> checkpoint the page
//          -> next page, newest -> oldest
//
// A large mailbox must never be acquired as one giant in-memory operation. The
// mailbox may hold 200,000 messages and this behaves exactly as if it held 200 -
// it simply takes more replay-safe pages.
//
// Usage:
//   node --env-file=.env.local --import tsx scripts/gmail-mailbox-intake.ts <dev|prod> [options]
//
//   --verify                 prove access only: refresh token, assert the profile,
//                            list one bounded page, read a few metadata records.
//                            ZERO database writes.
//   --incremental            recent overlapping window (GMAIL_INTAKE_INCREMENTAL_DAYS,
//                            default 7) instead of the monthly historical shards.
//   --since <YYYY-MM-DD>     oldest month to backfill (default GMAIL_INTAKE_SINCE,
//                            else 2011-06-01).
//   --shard <YYYY-MM>        run exactly one date shard.
//
// Environment:
//   GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET / GOOGLE_REFRESH_TOKEN  (required)
//   GMAIL_MAILBOX_ADDRESS   expected mailbox; default penfield33@gmail.com
//
// Error policy: a failed individual message is reported with its source id and
// the page continues. A failed page does NOT advance the checkpoint and the run
// exits non-zero; a re-run resumes the same page. Nothing is ever deleted.
// ---------------------------------------------------------------------------

import {
  getIntakeCheckpoint,
  saveIntakeCheckpoint,
  type IntakeCheckpoint,
  type SaveIntakeCheckpointInput,
} from '../db/intake-checkpoint'
import { landEmail } from '../db/landing'
import type { QueryExecutor } from '../db/query-executor'
import type { GmailMetadataMessage } from '../lib/relationship-intel/gmail-latest-context'
import {
  GMAIL_API_BASE,
  DEFAULT_GMAIL_MAILBOX,
  assertGmailAccount,
  gmailListPath,
  gmailMetadataPath,
  gmailMetadataToLandedEmail,
} from '../lib/intake/gmail'
import {
  GMAIL_INTAKE_SOURCE,
  monthShards,
  recentWindowShard,
  runGmailShard,
  type DateShard,
  type GmailTransport,
  type MailboxRunStats,
} from '../lib/intake/mailbox-paging'
import { createPoolExecutor } from './lib/pool-executor'

type EnvTarget = 'dev' | 'prod'

const DEFAULT_BACKFILL_SINCE = '2011-06-01'
const DEFAULT_INCREMENTAL_DAYS = 7
const VERIFY_SAMPLE_SIZE = 5

function requiredEnv(key: string): string {
  const value = process.env[key]?.trim()
  if (!value) throw new Error(`Missing required environment variable: ${key}`)
  return value
}

function targetDatabaseUrl(target: EnvTarget): string {
  return requiredEnv(target === 'prod' ? 'DATABASE_URL_PROD' : 'DATABASE_URL_DEV')
}

export function expectedMailbox(): string {
  return (process.env.GMAIL_MAILBOX_ADDRESS?.trim().toLowerCase() || DEFAULT_GMAIL_MAILBOX)
}

// --- authentication (reused from the proven gmail-metadata-sync flow) --------

export async function fetchAccessToken(): Promise<string> {
  const response = await fetch('https://oauth2.googleapis.com/token', {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      client_id: requiredEnv('GOOGLE_CLIENT_ID'),
      client_secret: requiredEnv('GOOGLE_CLIENT_SECRET'),
      refresh_token: requiredEnv('GOOGLE_REFRESH_TOKEN'),
      grant_type: 'refresh_token',
    }),
  })
  const json = (await response.json()) as { access_token?: string; error?: string }
  if (!response.ok || !json.access_token) {
    throw new Error(
      `Google token refresh failed${json.error ? ` (${json.error})` : ''}. The refresh token must include gmail.readonly scope.`,
    )
  }
  return json.access_token
}

export async function gmailProfile(token: string): Promise<{ emailAddress: string }> {
  const response = await fetch(`${GMAIL_API_BASE}/profile`, {
    headers: { authorization: `Bearer ${token}`, accept: 'application/json' },
  })
  if (!response.ok) {
    const detail = (await response.text()).slice(0, 300)
    throw new Error(`Gmail API ${response.status}: ${detail}`)
  }
  return (await response.json()) as { emailAddress: string }
}

/** REST transport: bounded list pages, metadata-only gets. */
export function createGmailTransport(accessToken: string): GmailTransport {
  async function apiGet<T>(path: string): Promise<T> {
    const response = await fetch(`${GMAIL_API_BASE}/${path}`, {
      headers: { authorization: `Bearer ${accessToken}`, accept: 'application/json' },
    })
    if (!response.ok) {
      const detail = (await response.text()).slice(0, 300)
      throw new Error(`Gmail API ${response.status}: ${detail}`)
    }
    return (await response.json()) as T
  }

  return {
    async listPage(query, pageToken) {
      const data = await apiGet<{ messages?: Array<{ id?: string }>; nextPageToken?: string }>(
        gmailListPath(query, pageToken),
      )
      const ids = (data.messages ?? [])
        .map((message) => message.id)
        .filter((id): id is string => Boolean(id))
      return { ids, nextPageToken: data.nextPageToken ?? null }
    },
    async getMetadata(id) {
      return apiGet<GmailMetadataMessage>(gmailMetadataPath(id))
    },
  }
}


// --- verify mode -------------------------------------------------------------

/**
 * Cheap access proof. Refreshes the token, asserts the profile, lists ONE bounded
 * page and reads a handful of metadata records. Zero database writes.
 */
export async function runVerify(): Promise<void> {
  const expected = expectedMailbox()
  const token = await fetchAccessToken()
  const profile = await gmailProfile(token)
  assertGmailAccount(profile.emailAddress, expected)
  console.log(`gmail [${expected}] profile verified`)

  const transport = createGmailTransport(token)
  const query = `after:1970/01/01 -in:trash -in:spam -label:drafts`
  const page = await transport.listPage(query, null)
  console.log(`gmail [${expected}] verify: one bounded page returned ${page.ids.length} id(s) (bound 500)`)
  for (const id of page.ids.slice(0, VERIFY_SAMPLE_SIZE)) {
    const message = await transport.getMetadata(id)
    console.log(
      `gmail [${expected}] verify: metadata ok id=${message.id} thread=${message.threadId ?? '-'} headersOnly=yes`,
    )
  }
  console.log('verify complete: 0 database writes')
}

// --- shard selection ---------------------------------------------------------

export function selectShards(opts: {
  incremental: boolean
  sinceIso: string
  now?: Date
  shardKey?: string | null
}): DateShard[] {
  if (opts.incremental) {
    const days = Number(process.env.GMAIL_INTAKE_INCREMENTAL_DAYS ?? DEFAULT_INCREMENTAL_DAYS)
    const shard = recentWindowShard(Number.isFinite(days) && days > 0 ? days : DEFAULT_INCREMENTAL_DAYS, opts.now)
    return [shard]
  }
  const shards = monthShards({ fromIso: opts.sinceIso, toIso: (opts.now ?? new Date()).toISOString() })
  if (opts.shardKey) {
    const only = shards.find((shard) => shard.key === opts.shardKey)
    if (!only) throw new Error(`shard ${opts.shardKey} is outside the requested range (since ${opts.sinceIso})`)
    return [only]
  }
  return shards
}

function valueAfter(args: string[], flag: string): string | null {
  const index = args.indexOf(flag)
  return index >= 0 ? args[index + 1] ?? null : null
}

// --- run ---------------------------------------------------------------------

export async function runGmailMailboxIntake(
  target: EnvTarget,
  execute: QueryExecutor,
  opts: { incremental: boolean; sinceIso: string; shardKey: string | null },
): Promise<MailboxRunStats[]> {
  const expected = expectedMailbox()
  const token = await fetchAccessToken()
  const profile = await gmailProfile(token)
  assertGmailAccount(profile.emailAddress, expected)

  const transport = createGmailTransport(token)
  // Incremental runs always start from the first page of a fresh overlap window,
  // so a stored cursor is ignored (landing is idempotent, so overlap is safe).
  const loadCheckpoint = async (shardKey: string): Promise<IntakeCheckpoint | null> =>
    opts.incremental ? null : getIntakeCheckpoint(GMAIL_INTAKE_SOURCE, expected, shardKey, execute)
  const saveCheckpoint = async (checkpoint: SaveIntakeCheckpointInput): Promise<void> => {
    await saveIntakeCheckpoint(checkpoint, execute)
  }
  const land = async (message: GmailMetadataMessage): Promise<boolean> => {
    const input = gmailMetadataToLandedEmail(message, expected)
    if (!input) throw new Error(`Gmail message ${message.id ?? '<no id>'} has no usable source id`)
    return landEmail(input, execute)
  }

  const shards = selectShards({
    incremental: opts.incremental,
    sinceIso: opts.sinceIso,
    shardKey: opts.shardKey,
  })
  console.log(
    `gmail [${expected}] target=${target} mode=${opts.incremental ? 'incremental' : 'historical'} shards=${shards.length}`,
  )

  const results: MailboxRunStats[] = []
  for (const shard of shards) {
    results.push(
      await runGmailShard({ transport, sourceAccount: expected, land, loadCheckpoint, saveCheckpoint }, shard),
    )
  }

  const totals = results.reduce(
    (acc, r) => ({
      pages: acc.pages + r.pages,
      seen: acc.seen + r.recordsSeen,
      landed: acc.landed + r.recordsLanded,
      replayed: acc.replayed + r.recordsReplayed,
      errors: acc.errors + r.errors,
    }),
    { pages: 0, seen: 0, landed: 0, replayed: 0, errors: 0 },
  )
  console.log(
    `gmail complete: source=${GMAIL_INTAKE_SOURCE} account=${expected} shards=${results.length} pages=${totals.pages} ` +
      `records_seen=${totals.seen} landed=${totals.landed} replayed=${totals.replayed} errors=${totals.errors}`,
  )
  return results
}

// --- CLI ---------------------------------------------------------------------

async function main(): Promise<void> {
  const args = process.argv.slice(2)
  const target = args[0]
  if (target !== 'dev' && target !== 'prod') {
    throw new Error(
      'Usage: node --env-file=.env.local --import tsx scripts/gmail-mailbox-intake.ts <dev|prod> [--verify] [--incremental] [--since YYYY-MM-DD] [--shard YYYY-MM]',
    )
  }

  // Verify never opens a database connection: it must perform zero writes.
  if (args.includes('--verify')) {
    await runVerify()
    return
  }

  const incremental = args.includes('--incremental')
  const sinceIso = valueAfter(args, '--since') ?? process.env.GMAIL_INTAKE_SINCE?.trim() ?? DEFAULT_BACKFILL_SINCE
  const shardKey = valueAfter(args, '--shard')

  const pool = createPoolExecutor(targetDatabaseUrl(target))
  try {
    const results = await runGmailMailboxIntake(target, pool.execute, { incremental, sinceIso, shardKey })
    const errors = results.reduce((n, r) => n + r.errors, 0)
    if (errors > 0) {
      throw new Error(
        `Gmail intake finished with ${errors} message error(s). The affected pages were still checkpointed; re-run to retry.`,
      )
    }
  } finally {
    await pool.end()
  }
}

if (process.argv[1]?.endsWith('gmail-mailbox-intake.ts')) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
