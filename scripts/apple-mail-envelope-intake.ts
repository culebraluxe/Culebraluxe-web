// Bounded Apple Mail metadata intake using Mail's local Envelope Index SQLite DB.
//
// Reads only Inbox/Sent metadata for every account in MAIL_APP_ACCOUNTS. No bodies,
// snippets, attachments, or raw MIME. Mail.app scripting is used only to resolve each
// account UUID from its configured address; message reads never cross the AppleEvent
// bridge.
//
// The recurring command is one newest band, which is cheap and re-read every run so a
// sync picks up mail that has arrived since. Landing is replay-safe, so re-reading a
// window lands only what is genuinely new; if a run is ever suspect, wipe the landing
// table and re-land rather than trying to reconcile it.
//
// The window is read in DISCRETE BANDS: 0-1 is the last month, 1-3 is the two
// months before that, then 3-6 and 6-12. Each band owns half-open
// [since, before) bounds, so bands never overlap and never leave a gap, and each
// pass reads only its own mail rather than re-reading everything newer. Every band
// keeps its own checkpoint, so starting the last month does not block widening
// outward later, and a finished band is never re-read.
//
// Usage (run newest band first):
//   node --env-file=.env.local --import tsx scripts/apple-mail-envelope-intake.ts prod --band=0-1 --verify
//   node --env-file=.env.local --import tsx scripts/apple-mail-envelope-intake.ts prod --band=0-1
//   node --env-file=.env.local --import tsx scripts/apple-mail-envelope-intake.ts prod --band=1-3
//   node --env-file=.env.local --import tsx scripts/apple-mail-envelope-intake.ts prod --band=3-6
//   node --env-file=.env.local --import tsx scripts/apple-mail-envelope-intake.ts prod --band=6-12
//   node --env-file=.env.local --import tsx scripts/apple-mail-envelope-intake.ts prod --band=all
//
// --band=all walks 0-1, 1-3, 3-6, 6-12 in order and stops at the first failure.

import { spawn } from 'node:child_process'
import { homedir } from 'node:os'
import { join, resolve } from 'node:path'
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { createPoolExecutor } from './lib/pool-executor'
import { appleMailReplayId, landAppleMail } from '../db/landing'

type EnvTarget = 'dev' | 'prod'
type Band = '0-1' | '1-3' | '3-6' | '6-12'

const BANDS: readonly Band[] = ['0-1', '1-3', '3-6', '6-12']
const BAND_LABEL: Record<Band, string> = {
  '0-1': 'last 1 month',
  '1-3': 'months 1-3 ago',
  '3-6': 'months 3-6 ago',
  '6-12': 'months 6-12 ago',
}

type Cursor = {
  date: number
  rowid: number
}

type LocalMailAddress = {
  address: string | null
  name: string | null
}

type LocalMailRecord = {
  mailbox: 'inbox' | 'sent'
  mailboxName: string
  localId: number
  messageId: string | null
  occurredAt: string | null
  sender: string | null
  to: LocalMailAddress[]
  cc: LocalMailAddress[]
  bcc: LocalMailAddress[]
  subject: string | null
}

type ExtractPage = {
  ok: true
  source: 'apple_mail_envelope_index'
  account: string
  accountId: string
  mailVersion: string
  database: string
  band: Band
  since: string
  before: string | null
  pageSize: number
  epoch: 'coredata' | 'unix'
  mailboxes: { inbox: string[]; sent: string[] }
  records: LocalMailRecord[]
  nextCursor: Cursor | null
  complete: boolean
  verify: boolean
}

type Checkpoint = {
  version: 1
  source: 'apple_mail_envelope_index'
  account: string
  accountId: string
  band: Band
  since: string
  before: string | null
  cursor: Cursor | null
  pagesCompleted: number
  recordsSeen: number
  landed: number
  replayed: number
  complete: boolean
  updatedAt: string
}

function requiredEnv(key: string): string {
  const value = process.env[key]?.trim()
  if (!value) throw new Error(`Missing required environment variable: ${key}`)
  return value
}

function targetDatabaseUrl(target: EnvTarget): string {
  return requiredEnv(target === 'prod' ? 'DATABASE_URL_PROD' : 'DATABASE_URL_DEV')
}

function parseTarget(raw: string | undefined): EnvTarget {
  if (raw === 'dev' || raw === 'prod') return raw
  throw new Error('Usage: apple-mail-envelope-intake.ts <dev|prod> --band=0-1|1-3|3-6|6-12|all [--verify] [--reset-checkpoint] [--max-pages=N]')
}

function option(name: string): string | null {
  const direct = process.argv.find((arg) => arg.startsWith(`${name}=`))
  if (direct) return direct.slice(name.length + 1)
  const index = process.argv.indexOf(name)
  if (index >= 0 && process.argv[index + 1] && !process.argv[index + 1].startsWith('--')) {
    return process.argv[index + 1]
  }
  return null
}

function parseBand(): Band | 'all' {
  const raw = (option('--band') ?? process.env.MAIL_SYNC_BAND?.trim() ?? '0-1').toLowerCase()
  if (raw === 'all') return 'all'
  if ((BANDS as readonly string[]).includes(raw)) return raw as Band
  throw new Error(`--band must be one of ${BANDS.join(', ')}, or all (got ${raw})`)
}

function parsePositiveInt(name: string, fallback: number): number {
  const raw = option(name)
  if (raw == null) return fallback
  const value = Number(raw)
  if (!Number.isInteger(value) || value <= 0) throw new Error(`${name} must be a positive integer`)
  return value
}

async function runProcess(command: string, args: string[]): Promise<{ stdout: string; stderr: string }> {
  return new Promise((resolveProcess, reject) => {
    const child = spawn(command, args, { stdio: ['ignore', 'pipe', 'pipe'] })
    let stdout = ''
    let stderr = ''
    child.stdout?.setEncoding('utf8')
    child.stderr?.setEncoding('utf8')
    child.stdout?.on('data', (chunk: string) => { stdout += chunk })
    child.stderr?.on('data', (chunk: string) => { stderr += chunk })
    child.on('error', reject)
    child.on('close', (code) => {
      if (code !== 0) {
        reject(new Error(`${command} failed (exit ${code}): ${stderr.trim() || stdout.trim()}`))
        return
      }
      resolveProcess({ stdout, stderr })
    })
  })
}

async function resolveAccountId(account: string): Promise<string> {
  const override = process.env.APPLE_MAIL_ACCOUNT_ID?.trim()
  if (override) return override
  const script = resolve(process.cwd(), 'scripts/macbridge/apple-mail-account-id.jxa')
  const { stdout } = await runProcess('/usr/bin/osascript', ['-l', 'JavaScript', script, account])
  const id = stdout.trim()
  if (!id) throw new Error(`Mail.app returned an empty account id for ${account}`)
  return id
}

async function extractPage(input: {
  account: string
  accountId: string
  band: Band
  pageSize: number
  cursor: Cursor | null
  verify: boolean
}): Promise<ExtractPage> {
  const extractor = resolve(process.cwd(), 'scripts/macbridge/apple-mail-envelope-sqlite.py')
  const args = [
    'python3',
    extractor,
    '--account', input.account,
    '--account-id', input.accountId,
    '--band', input.band,
    '--limit', String(input.verify ? Math.min(20, input.pageSize) : input.pageSize),
  ]
  if (input.cursor) {
    args.push('--cursor-date', String(input.cursor.date), '--cursor-rowid', String(input.cursor.rowid))
  }
  if (input.verify) args.push('--verify')
  const { stdout, stderr } = await runProcess('/usr/bin/env', args)
  if (stderr.trim()) process.stderr.write(stderr)
  let parsed: unknown
  try {
    parsed = JSON.parse(stdout)
  } catch {
    throw new Error(`Apple Mail SQLite extractor returned invalid JSON: ${stdout.slice(0, 500)}`)
  }
  if (!parsed || typeof parsed !== 'object' || (parsed as { ok?: boolean }).ok !== true) {
    throw new Error(`Apple Mail SQLite extractor failed: ${stdout.slice(0, 500)}`)
  }
  return parsed as ExtractPage
}

function configuredAccounts(): string[] {
  const configured = process.env.MAIL_APP_ACCOUNTS?.trim()
  const raw = configured
    ? configured.split(',')
    : [
        process.env.APPLE_MAILBOX_ADDRESS?.trim() ||
          process.env.ICLOUD_MAIL_ADDRESS?.trim() ||
          '',
      ]
  const accounts = raw.map((entry) => entry.trim().toLowerCase()).filter(Boolean)
  if (accounts.length === 0) {
    throw new Error('MAIL_APP_ACCOUNTS is set but lists no accounts, and no fallback address is configured')
  }
  return [...new Set(accounts)]
}

function checkpointPath(account: string, band: Band): string {
  const safe = account.toLowerCase().replace(/[^a-z0-9._-]+/g, '_')
  // The band is part of the checkpoint identity: a band that has finished is
  // never re-read, and a wider band can never be skipped because a narrower one
  // already completed.
  return join(homedir(), '.culebraluxe', 'intake-checkpoints', `applemail-${safe}-band-${band}.json`)
}

async function readCheckpoint(path: string): Promise<Checkpoint | null> {
  try {
    return JSON.parse(await readFile(path, 'utf8')) as Checkpoint
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code
    if (code === 'ENOENT') return null
    throw error
  }
}

async function writeCheckpoint(path: string, state: Checkpoint): Promise<void> {
  await mkdir(join(path, '..'), { recursive: true })
  const temp = `${path}.${process.pid}.tmp`
  await writeFile(temp, `${JSON.stringify(state, null, 2)}\n`, { encoding: 'utf8', mode: 0o600 })
  await rename(temp, path)
}

async function verify(account: string, accountId: string, band: Band, pageSize: number): Promise<void> {
  const page = await extractPage({ account, accountId, band, pageSize, cursor: null, verify: true })
  console.log('Apple Mail Envelope Index verification OK')
  console.log(`account=${page.account}`)
  console.log(`account_id=${page.accountId}`)
  console.log(`mail_version=${page.mailVersion}`)
  console.log(`band=${band} (${BAND_LABEL[band]})`)
  console.log(`window=${page.since} .. ${page.before ?? 'now'}`)
  console.log(`inbox=${page.mailboxes.inbox.join(', ')}`)
  console.log(`sent=${page.mailboxes.sent.join(', ')}`)
  console.log(`sample_records=${page.records.length}`)
  console.log('database_writes=0')
}

async function intake(target: EnvTarget, account: string, accountId: string, band: Band): Promise<void> {
  const pageSize = Math.min(parsePositiveInt('--page-size', 500), 1000)
  const maxPagesRaw = option('--max-pages')
  const maxPages = maxPagesRaw ? parsePositiveInt('--max-pages', 1) : null
  const path = checkpointPath(account, band)

  if (process.argv.includes('--reset-checkpoint')) {
    await rm(path, { force: true })
  }

  let checkpoint = await readCheckpoint(path)
  if (checkpoint && (checkpoint.account !== account || checkpoint.accountId !== accountId || checkpoint.band !== band)) {
    throw new Error(`Checkpoint does not match the current source: ${path}`)
  }
  // Deliberately NOT skipping a band marked complete. Every band slides with "now",
  // so the newest band must be re-read to pick up mail that arrived since the last
  // run; landing is replay-safe, so the only cost is the read. The checkpoint earns
  // its keep by resuming an interrupted run, and the extractor discards a cursor
  // that has fallen outside the current window.

  const pool = createPoolExecutor(targetDatabaseUrl(target))
  try {
    let pagesThisRun = 0
    while (true) {
      const cursor = checkpoint?.cursor ?? null
      const page = await extractPage({ account, accountId, band, pageSize, cursor, verify: false })
      let landed = 0
      let replayed = 0

      for (const record of page.records) {
        const sourceMessageId = appleMailReplayId({
          messageId: record.messageId,
          mailboxKind: record.mailbox,
          localId: record.localId,
        })
        if (!sourceMessageId) {
          throw new Error(`Apple Mail row ${record.localId} has no stable replay identity`)
        }

        const inserted = await landAppleMail(
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
          pool.execute,
        )
        if (inserted) landed += 1
        else replayed += 1
      }

      const next: Checkpoint = {
        version: 1,
        source: 'apple_mail_envelope_index',
        account,
        accountId,
        band,
        since: checkpoint?.since ?? page.since,
        before: checkpoint?.before ?? page.before,
        cursor: page.nextCursor,
        pagesCompleted: (checkpoint?.pagesCompleted ?? 0) + 1,
        recordsSeen: (checkpoint?.recordsSeen ?? 0) + page.records.length,
        landed: (checkpoint?.landed ?? 0) + landed,
        replayed: (checkpoint?.replayed ?? 0) + replayed,
        complete: page.complete,
        updatedAt: new Date().toISOString(),
      }

      // The checkpoint moves only after every row in this page has landed/replayed.
      await writeCheckpoint(path, next)
      checkpoint = next
      pagesThisRun += 1

      console.log(
        `applemail ${account} band=${band} page=${next.pagesCompleted} ` +
        `received=${page.records.length} landed=${landed} replayed=${replayed} ` +
        `complete=${page.complete ? 'yes' : 'no'}`,
      )

      if (page.complete) {
        console.log(
          `Apple Mail band ${band} (${BAND_LABEL[band]}) complete: pages=${next.pagesCompleted} ` +
          `records=${next.recordsSeen} landed=${next.landed} replayed=${next.replayed} checkpoint=${path}`,
        )
        return
      }

      if (!page.nextCursor) {
        throw new Error('Extractor returned an incomplete page without a continuation cursor')
      }

      if (maxPages !== null && pagesThisRun >= maxPages) {
        console.log(`Stopped cleanly after --max-pages=${maxPages}; rerun the same command to resume.`)
        return
      }
    }
  } finally {
    await pool.end()
  }
}

async function main() {
  const target = parseTarget(process.argv[2])
  const band = parseBand()
  const pageSize = Math.min(parsePositiveInt('--page-size', 500), 1000)
  const only = option('--account')
  // Every account in MAIL_APP_ACCOUNTS, the same list the previous reader used, so
  // the A/B sources (two Gmail, one iCloud, three very different mailbox sizes) are
  // all read by one command.
  const accounts = only ? [only.trim().toLowerCase()] : configuredAccounts()
  const bands: readonly Band[] = band === 'all' ? BANDS : [band]

  const failures: string[] = []
  for (const account of accounts) {
    let accountId: string
    try {
      accountId = await resolveAccountId(account)
    } catch (error) {
      // One account that is not configured in Mail.app, or that Mail refuses to
      // answer for, must not stop the others: a daily sync that dies on the first
      // account is useless. Report it and keep going, then exit non-zero.
      failures.push(`${account}: ${error instanceof Error ? error.message : String(error)}`)
      continue
    }

    if (process.argv.includes('--verify')) {
      try {
        await verify(account, accountId, bands[0], pageSize)
      } catch (error) {
        failures.push(`${account} verify: ${error instanceof Error ? error.message : String(error)}`)
      }
      continue
    }

    for (const current of bands) {
      try {
        await intake(target, account, accountId, current)
      } catch (error) {
        failures.push(`${account} band ${current}: ${error instanceof Error ? error.message : String(error)}`)
        break
      }
    }
  }

  if (failures.length > 0) {
    for (const failure of failures) console.error(`applemail FAILED ${failure}`)
    process.exitCode = 1
  }
}

if (process.argv[1]?.endsWith('apple-mail-envelope-intake.ts')) {
  main().catch((error) => {
    console.error(`Apple Mail Envelope Index intake failed: ${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  })
}
