// ---------------------------------------------------------------------------
// APPLE MAILBOX INTAKE — bounded, resumable, replay-safe.
//
// Wraps the EXISTING PROVEN Apple acquisition/export mechanism
// (scripts/macbridge/apple-mail-metadata.jxa, the authenticated Mail.app bridge)
// in BOUNDED extraction. The source authority and query are unchanged; only a
// page size and cursor are added, so the exporter can never hand back unbounded
// mailbox history:
//
//   Apple source -> fetch 500 -> land 500 -> checkpoint cursor -> fetch next 500
//
// Every valid source record lands in l_applemail BEFORE any interpretation. No
// Person decision, no filtering, no ambiguous-recipient rejection at landing.
//
// Usage:
//   node --env-file=.env.local --import tsx scripts/apple-mailbox-intake.ts <dev|prod> [options]
//
//   --verify                 open the source, request ONE bounded page, confirm
//                            records can be produced. ZERO database writes.
//   --since <iso|all>        window passed to the proven exporter (default
//                            APPLE_MAILBOX_SINCE, else all).
//   --mailbox <inbox|sent>   run exactly one mailbox (default: both, in order).
//
// Environment:
//   APPLE_MAILBOX_ADDRESS    the mailbox this intake is scoped to; default
//                            lisa@culebraluxe.com
//   APPLE_MAILBOX_SINCE      optional default window
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
  serializeAppleCursor,
  type AppleLocalMailRecord,
  type AppleMailboxKind,
  type AppleMailboxPage,
  type AppleMailTransport,
  type AppleMailCursor,
  type MailboxRunStats,
} from '../lib/intake/mailbox-paging'
import { runAppleMailboxShard } from '../lib/intake/mailbox-paging'
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

export function mailboxAddress(): string {
  return process.env.APPLE_MAILBOX_ADDRESS?.trim().toLowerCase() || DEFAULT_APPLE_MAILBOX
}

function mailboxWindow(): string {
  return process.env.APPLE_MAILBOX_SINCE?.trim() || 'all'
}

// --- bounded transport (one page per exporter invocation) --------------------

type ExporterPageResult = {
  mailboxKind: string
  nextCursor: Record<string, AppleMailCursor> | null
  complete: boolean
  emitted: number
  scanned: number
}

/** Parse the JSONL the exporter produced into records. */
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
 * Run the PROVEN exporter for exactly ONE bounded page and return it.
 *
 * The exporter itself enforces the page: argv carries the page size, the mailbox
 * and the cursor, and the sidecar file it writes reports nextCursor/complete. The
 * caller never receives unbounded mailbox history.
 */
export async function runExporterPage(
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
      const exporter = resolve(process.cwd(), 'scripts/macbridge/apple-mail-metadata.jxa')
      const child = spawn(
        '/usr/bin/osascript',
        [
          '-l',
          'JavaScript',
          exporter,
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
          reject(new Error(`Mail.app metadata exporter failed (exit ${code}): ${stderr.trim()}`))
          return
        }
        resolveProcess()
      })
    })

    const records = parseRecords(await readFile(outputPath, 'utf8'))
    const page = JSON.parse(await readFile(pagePath, 'utf8')) as ExporterPageResult
    if (records.length > pageSize) {
      throw new Error(`Exporter returned ${records.length} records for a ${pageSize}-record page; acquisition must be bounded`)
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

export function createAppleTransport(account: string, opts: { pageSize?: number; since?: string } = {}): AppleMailTransport {
  return {
    fetchPage(mailboxKind, cursor) {
      return runExporterPage(account, mailboxKind, cursor, opts)
    },
  }
}

export type AppleVerifyResult = { mailboxKind: AppleMailboxKind; received: number; complete: boolean }

/**
 * Verify mode: request ONE bounded page and confirm records can be produced.
 * Performs ZERO database writes - it never touches a landing or checkpoint writer.
 */
export async function runAppleVerify(
  transport: AppleMailTransport,
  mailboxKind: AppleMailboxKind = 'inbox',
): Promise<AppleVerifyResult> {
  const page = await transport.fetchPage(mailboxKind, null)
  return { mailboxKind, received: page.records.length, complete: page.complete }
}


// --- run ---------------------------------------------------------------------

/**
 * Land one exporter record. Every VALID source record lands before any
 * interpretation - no Person decision, no filtering, no ambiguous-recipient
 * rejection. A record with neither an RFC Message-ID nor a real stable Mail.app
 * local id has no replay identity and is reported as unlandable, never invented.
 */
export async function landAppleRecord(
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
      `unlandable Apple record: no Message-ID and no stable Mail.app local id (mailbox=${record.mailboxName})`,
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

export async function runAppleMailboxIntake(
  target: EnvTarget,
  execute: QueryExecutor,
  opts: { kinds: AppleMailboxKind[]; since?: string },
): Promise<MailboxRunStats[]> {
  const account = mailboxAddress()
  const transport = createAppleTransport(account, { since: opts.since ?? mailboxWindow() })
  const land = (record: AppleLocalMailRecord) => landAppleRecord(record, account, execute)
  const loadCheckpoint = (shardKey: string): Promise<IntakeCheckpoint | null> =>
    getIntakeCheckpoint(APPLE_MAIL_INTAKE_SOURCE, account, shardKey, execute)
  const saveCheckpoint = async (checkpoint: SaveIntakeCheckpointInput): Promise<void> => {
    await saveIntakeCheckpoint(checkpoint, execute)
  }

  console.log(`applemail [${account}] target=${target} mailboxes=${opts.kinds.join(',')} page_size=${APPLE_PAGE_SIZE}`)

  const results: MailboxRunStats[] = []
  for (const kind of opts.kinds) {
    results.push(await runAppleMailboxShard({ transport, sourceAccount: account, land, loadCheckpoint, saveCheckpoint }, kind))
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
    `applemail complete: source=${APPLE_MAIL_INTAKE_SOURCE} account=${account} mailboxes=${results.length} pages=${totals.pages} ` +
      `records_seen=${totals.seen} landed=${totals.landed} replayed=${totals.replayed} errors=${totals.errors}`,
  )
  return results
}

// --- CLI ---------------------------------------------------------------------

function valueAfter(args: string[], flag: string): string | null {
  const index = args.indexOf(flag)
  return index >= 0 ? args[index + 1] ?? null : null
}

async function main(): Promise<void> {
  const args = process.argv.slice(2)
  const target = args[0]
  if (target !== 'dev' && target !== 'prod') {
    throw new Error(
      'Usage: node --env-file=.env.local --import tsx scripts/apple-mailbox-intake.ts <dev|prod> [--verify] [--since iso|all] [--mailbox inbox|sent]',
    )
  }
  const account = mailboxAddress()
  const since = valueAfter(args, '--since') ?? mailboxWindow()
  const mailboxArg = (valueAfter(args, '--mailbox') ?? '').trim().toLowerCase()
  const kinds: AppleMailboxKind[] =
    mailboxArg === 'inbox' || mailboxArg === 'sent' ? [mailboxArg] : APPLE_MAILBOX_KINDS

  // Verify never opens a database connection: it must perform zero writes.
  if (args.includes('--verify')) {
    const transport = createAppleTransport(account, { since })
    const result = await runAppleVerify(transport, kinds[0])
    console.log(
      `applemail [${account}] verify: mailbox=${result.mailboxKind} received=${result.received} complete=${result.complete} | 0 database writes`,
    )
    return
  }

  const pool = createPoolExecutor(targetDatabaseUrl(target))
  try {
    const results = await runAppleMailboxIntake(target, pool.execute, { kinds, since })
    const errors = results.reduce((n, r) => n + r.errors, 0)
    if (errors > 0) {
      throw new Error(
        `Apple intake finished with ${errors} record error(s). The affected pages were still checkpointed; re-run to retry.`,
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
