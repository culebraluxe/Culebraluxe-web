// ---------------------------------------------------------------------------
// MAILBOX INTAKE - headers off the LOCAL BOX, into the L table.
//
// This is the working mechanism, unchanged in spirit from the 2026-09-02
// version: Apple Mail is signed in to every mailbox we care about (the Gmail
// accounts included), so the mail is already on disk. The authenticated Mail.app
// bridge (scripts/macbridge/apple-mail-metadata.jxa) reads HEADER information
// out of it and this script writes it to the landing table. No Gmail API, no
// OAuth client, no refresh token.
//
//   local box -> bounded page (500) -> land immediately -> checkpoint -> next
//
// Every configured Mail.app account is pulled, oldest-history included (default
// window is `all`, i.e. the full local box - not a 90-day slice). The mailbox
// may hold 200,000 messages and this behaves exactly as if it held 200; it just
// takes more replay-safe pages, and it can be resumed after a crash.
//
// Header information only: participants, timestamp, subject and Message-ID/local
// provenance. Never bodies, snippets, attachments or raw MIME.
//
// Usage:
//   node --env-file=.env.local --import tsx scripts/apple-mailbox-intake.ts <dev|prod> [options]
//
//   --verify                 open the source, request ONE bounded page, confirm
//                            records can be produced. ZERO database writes.
//   --since <iso|all>        window for the reader (default APPLE_MAILBOX_SINCE,
//                            else all - the whole local box).
//   --mailbox <inbox|sent>   run exactly one mailbox kind (default: both).
//   --account <email>        run exactly one Mail.app account.
//
// Environment:
//   MAIL_APP_ACCOUNTS        comma-separated accounts to pull (all of them)
//                            fallback: APPLE_MAILBOX_ADDRESS, then
//                            ICLOUD_MAIL_ADDRESS, then lisa@culebraluxe.com
//   APPLE_MAILBOX_SINCE      optional default window (default all)
//
// Error policy: a failed record is reported with its source id and the page
// continues. A failed page does NOT advance the checkpoint and the run exits
// non-zero; a re-run resumes the same page. Nothing is deleted.
// ---------------------------------------------------------------------------

import { spawn } from 'node:child_process'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import {
  getIntakeCheckpoint,
  saveIntakeCheckpoint,
  type IntakeCheckpoint,
  type SaveIntakeCheckpointInput,
} from '../db/intake-checkpoint'
import { appleMailReplayId, landAppleMail } from '../db/landing'
import type { QueryExecutor } from '../db/query-executor'
import {
  APPLE_MAIL_INTAKE_SOURCE,
  APPLE_PAGE_SIZE,
  runAppleMailboxShard,
  serializeAppleCursor,
  type AppleLocalMailRecord,
  type AppleMailCursor,
  type AppleMailboxKind,
  type AppleMailboxPage,
  type AppleMailTransport,
  type MailboxRunStats,
} from '../lib/intake/mailbox-paging'
import { createPoolExecutor } from './lib/pool-executor'

type EnvTarget = 'dev' | 'prod'

export const DEFAULT_APPLE_MAILBOX = 'lisa@culebraluxe.com'
export const APPLE_MAILBOX_KINDS: AppleMailboxKind[] = ['inbox', 'sent']

function requiredEnv(key: string): string {
  const value = process.env[key]?.trim()
  if (!value) throw new Error(`Missing required environment variable: ${key}`)
  return value
}

function targetDatabaseUrl(target: EnvTarget): string {
  return requiredEnv(target === 'prod' ? 'DATABASE_URL_PROD' : 'DATABASE_URL_DEV')
}

/** Every Mail.app account to pull, in order. Mail.app already holds the mail. */
export function mailboxAccounts(): string[] {
  const configured = process.env.MAIL_APP_ACCOUNTS?.trim()
  const raw = configured
    ? configured.split(',')
    : [
        process.env.APPLE_MAILBOX_ADDRESS?.trim() ||
          process.env.ICLOUD_MAIL_ADDRESS?.trim() ||
          DEFAULT_APPLE_MAILBOX,
      ]
  const accounts = raw.map((entry) => entry.trim().toLowerCase()).filter(Boolean)
  if (accounts.length === 0) throw new Error('MAIL_APP_ACCOUNTS is set but lists no accounts.')
  return [...new Set(accounts)]
}

/** The whole local box by default - the window/cap that silently truncated it is gone. */
export function mailboxWindow(): string {
  return process.env.APPLE_MAILBOX_SINCE?.trim() || 'all'
}

// --- bounded transport (one page per reader invocation) ----------------------

type ReaderPageResult = {
  mailboxKind: string
  nextCursor: Record<string, AppleMailCursor> | null
  complete: boolean
  emitted: number
  scanned: number
}

/** Parse the JSONL the reader produced into records. */
export function parseRecords(jsonl: string): AppleLocalMailRecord[] {
  return jsonl
    .split(/\r?\n/)
    .filter((line) => line.trim().length > 0)
    .map((line, index) => {
      try {
        return JSON.parse(line) as AppleLocalMailRecord
      } catch {
        throw new Error(`Invalid Mail.app metadata JSON on line ${index + 1}`)
      }
    })
}

/**
 * Run the reader for exactly ONE bounded page and return it.
 *
 * argv carries the page size, the mailbox and the cursor; the sidecar file it
 * writes reports nextCursor/complete. The caller never receives unbounded
 * mailbox history.
 */
export async function runReaderPage(
  account: string,
  mailboxKind: AppleMailboxKind,
  cursor: AppleMailCursor | null,
  opts: { pageSize?: number; since?: string } = {},
): Promise<AppleMailboxPage> {
  const pageSize = opts.pageSize ?? APPLE_PAGE_SIZE
  const since = opts.since ?? mailboxWindow()
  const temporaryDirectory = await mkdtemp(join(tmpdir(), 'culebraluxe-mailbox-'))
  const outputPath = join(temporaryDirectory, 'messages.jsonl')
  const pagePath = `${outputPath}.page.json`
  await writeFile(outputPath, '', 'utf8')
  try {
    let stderr = ''
    await new Promise<void>((resolveProcess, reject) => {
      const reader = resolve(process.cwd(), 'scripts/macbridge/apple-mail-metadata.jxa')
      const child = spawn(
        '/usr/bin/osascript',
        [
          '-l',
          'JavaScript',
          reader,
          account,
          outputPath,
          // Positional so a window without a cap cannot shift argv.
          since,
          String(pageSize),
          mailboxKind,
          cursor ? serializeAppleCursor(cursor) : 'none',
        ],
        { stdio: ['ignore', 'ignore', 'pipe'] },
      )
      child.stderr?.setEncoding('utf8')
      child.stderr?.on('data', (chunk: string) => {
        stderr += chunk
        process.stderr.write(chunk)
      })
      child.on('error', reject)
      child.on('close', (code) => {
        if (code !== 0) {
          reject(new Error(`Mail.app metadata reader failed (exit ${code}): ${stderr.trim()}`))
          return
        }
        resolveProcess()
      })
    })

    const records = parseRecords(await readFile(outputPath, 'utf8'))
    const page = JSON.parse(await readFile(pagePath, 'utf8')) as ReaderPageResult
    if (records.length > pageSize) {
      throw new Error(`Reader returned ${records.length} records for a ${pageSize}-record page; acquisition must be bounded`)
    }
    return {
      records,
      nextCursor: page.complete || !page.nextCursor ? null : page.nextCursor[mailboxKind] ?? null,
      complete: Boolean(page.complete),
    }
  } finally {
    await rm(temporaryDirectory, { recursive: true, force: true })
  }
}

export function createMailTransport(account: string, opts: { pageSize?: number; since?: string } = {}): AppleMailTransport {
  return {
    fetchPage(mailboxKind, cursor) {
      return runReaderPage(account, mailboxKind, cursor, opts)
    },
  }
}

export type MailboxVerifyResult = {
  account: string
  mailboxKind: AppleMailboxKind
  received: number
  complete: boolean
}

/**
 * Verify mode: request ONE bounded page and confirm records can be produced.
 * Performs ZERO database writes - it never touches a landing or checkpoint writer.
 */
export async function runMailboxVerify(
  transport: AppleMailTransport,
  account: string,
  mailboxKind: AppleMailboxKind = 'inbox',
): Promise<MailboxVerifyResult> {
  const page = await transport.fetchPage(mailboxKind, null)
  return { account, mailboxKind, received: page.records.length, complete: page.complete }
}


// --- run ---------------------------------------------------------------------

/**
 * Land one record. Every VALID source record lands before any interpretation -
 * no Person decision, no filtering, no ambiguous-recipient rejection. A record
 * with neither an RFC Message-ID nor a real stable Mail.app local id has no
 * replay identity and is reported as unlandable, never invented.
 */
export async function landMailRecord(
  record: AppleLocalMailRecord,
  account: string,
  execute: QueryExecutor,
): Promise<boolean> {
  const sourceMessageId = appleMailReplayId({
    messageId: record.messageId,
    mailboxKind: record.mailbox,
    localId: record.localId,
  })
  if (!sourceMessageId) {
    throw new Error(
      `unlandable record: no Message-ID and no stable Mail.app local id (mailbox=${record.mailboxName})`,
    )
  }
  return landAppleMail(
    {
      sourceAccount: account,
      sourceMessageId,
      mailboxKind: record.mailbox,
      mailboxName: record.mailboxName,
      localId: record.localId,
      messageId: record.messageId,
      occurredAt: record.occurredAt,
      sender: record.sender,
      toRecipients: record.to,
      ccRecipients: record.cc,
      bccRecipients: record.bcc,
      subject: record.subject,
      raw: record,
    },
    execute,
  )
}

export type MailboxAccountFailure = { account: string; message: string }
export type MailboxIntakeOutcome = { results: MailboxRunStats[]; failures: MailboxAccountFailure[] }

export async function runMailboxIntake(
  target: EnvTarget,
  execute: QueryExecutor,
  opts: { accounts: string[]; kinds: AppleMailboxKind[]; since?: string },
): Promise<MailboxIntakeOutcome> {
  const since = opts.since ?? mailboxWindow()
  console.log(
    `mailbox intake: target=${target} accounts=${opts.accounts.join(',')} mailboxes=${opts.kinds.join(',')} window=${since} page_size=${APPLE_PAGE_SIZE}`,
  )

  const results: MailboxRunStats[] = []
  const failures: MailboxAccountFailure[] = []
  for (const account of opts.accounts) {
    // One unresponsive mailbox must not abort every other account: report it and
    // move on. Nothing already landed is touched, and the run still exits non-zero.
    try {
      const transport = createMailTransport(account, { since })
      const land = (record: AppleLocalMailRecord) => landMailRecord(record, account, execute)
      const loadCheckpoint = (shardKey: string): Promise<IntakeCheckpoint | null> =>
        getIntakeCheckpoint(APPLE_MAIL_INTAKE_SOURCE, account, shardKey, execute)
      const saveCheckpoint = async (checkpoint: SaveIntakeCheckpointInput): Promise<void> => {
        await saveIntakeCheckpoint(checkpoint, execute)
      }

      for (const kind of opts.kinds) {
        results.push(
          await runAppleMailboxShard({ transport, sourceAccount: account, land, loadCheckpoint, saveCheckpoint }, kind),
        )
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      failures.push({ account, message })
      console.error(`mailbox [${account}] FAILED: ${message} - continuing with the remaining accounts`)
    }
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
    `mailbox intake complete: source=${APPLE_MAIL_INTAKE_SOURCE} accounts=${opts.accounts.length} shards=${results.length} ` +
      `failed_accounts=${failures.length} pages=${totals.pages} records_seen=${totals.seen} landed=${totals.landed} ` +
      `replayed=${totals.replayed} errors=${totals.errors}`,
  )
  return { results, failures }
}

// --- CLI ---------------------------------------------------------------------

function valueAfter(args: string[], flag: string): string | null {
  const index = args.indexOf(flag)
  return index >= 0 ? args[index + 1] ?? null : null
}

function requestedKinds(args: string[]): AppleMailboxKind[] {
  const raw = (valueAfter(args, '--mailbox') ?? '').trim().toLowerCase()
  return raw === 'inbox' || raw === 'sent' ? [raw] : APPLE_MAILBOX_KINDS
}

function requestedAccounts(args: string[]): string[] {
  const explicit = (valueAfter(args, '--account') ?? '').trim().toLowerCase()
  return explicit ? [explicit] : mailboxAccounts()
}

async function main(): Promise<void> {
  const args = process.argv.slice(2)
  const target = args[0]
  if (target !== 'dev' && target !== 'prod') {
    throw new Error(
      'Usage: node --env-file=.env.local --import tsx scripts/apple-mailbox-intake.ts <dev|prod> [--verify] [--since iso|all] [--mailbox inbox|sent] [--account email]',
    )
  }
  const since = valueAfter(args, '--since') ?? mailboxWindow()
  const kinds = requestedKinds(args)
  const accounts = requestedAccounts(args)

  // Verify never opens a database connection: it must perform zero writes.
  if (args.includes('--verify')) {
    let failed = 0
    for (const account of accounts) {
      try {
        const transport = createMailTransport(account, { since })
        const result = await runMailboxVerify(transport, account, kinds[0])
        console.log(
          `verify [${result.account}] mailbox=${result.mailboxKind} received=${result.received} complete=${result.complete} | 0 database writes`,
        )
      } catch (error) {
        failed += 1
        console.error(`verify [${account}] FAILED: ${error instanceof Error ? error.message : String(error)}`)
      }
    }
    if (failed > 0) throw new Error(`${failed} account(s) could not be read from Mail.app`)
    return
  }

  const pool = createPoolExecutor(targetDatabaseUrl(target))
  try {
    const { results, failures } = await runMailboxIntake(target, pool.execute, { accounts, kinds, since })
    const errors = results.reduce((n, r) => n + r.errors, 0)
    if (errors > 0 || failures.length > 0) {
      throw new Error(
        `mailbox intake finished with ${errors} record error(s) and ${failures.length} failed account(s). ` +
          'The affected pages were still checkpointed; re-run to retry.',
      )
    }
  } finally {
    await pool.end()
  }
}

if (process.argv[1]?.endsWith('apple-mailbox-intake.ts')) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
