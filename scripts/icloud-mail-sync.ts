// Apple-hosted iCloud Mail metadata intake.
// Reads only authenticated Mail.app metadata: participants, timestamp, subject,
// and Message-ID/local provenance. It never requests bodies, snippets, attachments,
// or raw MIME and performs no mailbox mutations.
import { spawn } from 'node:child_process'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { createPoolExecutor } from './lib/pool-executor'
import type { QueryExecutor } from '../db/query-executor'
import {
  getRelationshipEvidenceRows,
  recordReconcileDecision,
  upsertRelationshipEvidence,
} from '../db/relationship-evidence'
import { createInteraction } from '../db/interactions'
import { appleMailReplayId, landAppleMail } from '../db/landing'
import { createInMemoryPersonLookup } from '../lib/relationship-intel/inmemory-lookup'
import { reconcileEvidence } from '../lib/relationship-intel/reconcile'
import {
  boundedEmailSubject,
  buildICloudMailEvidence,
  classifyEnvelope,
  ICLOUD_MAIL_SOURCE,
  normalizeMailbox,
  observationToInteraction,
  type ICloudMailObservation,
} from '../lib/relationship-intel/icloud-mail'

type EnvTarget = 'dev' | 'prod'

function requiredEnv(key: string): string {
  const value = process.env[key]?.trim()
  if (!value) throw new Error(`Missing required environment variable: ${key}`)
  return value
}

function targetDatabaseUrl(target: EnvTarget): string {
  return requiredEnv(target === 'prod' ? 'DATABASE_URL_PROD' : 'DATABASE_URL_DEV')
}

function internalAddresses(): Set<string> {
  const values = requiredEnv('EMAIL_INTERNAL_ADDRESSES').split(',')
  const normalized = values.map(normalizeMailbox).filter((value): value is string => Boolean(value))
  if (normalized.length !== values.length) {
    throw new Error('EMAIL_INTERNAL_ADDRESSES contains an invalid email address.')
  }
  return new Set(normalized)
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

function senderAddress(value: string | null): LocalMailAddress | null {
  if (!value) return null
  const bracketed = value.match(/^(.*?)<([^<>]+)>\\s*$/)
  const address = normalizeMailbox(bracketed?.[2] ?? value.match(/[^\\s<>]+@[^\\s<>]+/)?.[0] ?? '')
  if (!address) return null
  const name = bracketed?.[1]?.replace(/^["']|["']$/g, '').trim() || null
  return { address, name }
}

async function runMailExporter(account: string, window: MailWindow): Promise<LocalMailRecord[]> {
  const temporaryDirectory = await mkdtemp(join(tmpdir(), 'culebraluxe-mail-'))
  const outputPath = join(temporaryDirectory, 'messages.jsonl')
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
          // Always positional, so a window without a cap cannot shift argv.
          window.since ?? 'all',
          window.maxPerMailbox ? String(window.maxPerMailbox) : '0',
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

    const jsonl = await readFile(outputPath, 'utf8')
    return jsonl
      .split(/\r?\n/)
      .filter(Boolean)
      .map((line, index) => {
        try {
          return JSON.parse(line) as LocalMailRecord
        } catch {
          throw new Error(`Invalid Mail.app metadata JSON file on line ${index + 1}`)
        }
      })
  } finally {
    await rm(temporaryDirectory, { recursive: true, force: true })
  }
}

/**
 * Mail.app accounts to pull, in order.
 *
 * Plan B (2026-09-11): Apple Mail is ALREADY authenticated to Google, so Gmail
 * arrives through the same authenticated Mail.app bridge as iCloud. No OAuth
 * client, no consent screen, no refresh token to expire, and no contact with the
 * portal's login credentials (AUTH_GOOGLE_ID / AUTH_GOOGLE_SECRET are never read
 * here). The acquisition channel - not the provider - decides the landing table,
 * so every account below lands in l_applemail keyed on its own source_account.
 *
 * Falls back to the legacy single-account ICLOUD_MAIL_ADDRESS so existing runs are
 * unchanged.
 */
function mailAppAccounts(): string[] {
  const configured = process.env.MAIL_APP_ACCOUNTS?.trim()
  const raw = configured ? configured.split(',') : [requiredEnv('ICLOUD_MAIL_ADDRESS')]
  const accounts = raw.map((entry) => entry.trim().toLowerCase()).filter(Boolean)
  if (accounts.length === 0) throw new Error('MAIL_APP_ACCOUNTS is set but lists no accounts.')
  return [...new Set(accounts)]
}

type MailWindow = {
  since: string | null
  maxPerMailbox: number | null
}

const DEFAULT_MAIL_SYNC_SINCE = '90d'

/**
 * How far back the current pull reaches.
 *
 * MEASURED: Mail.app scripting yields ~5.3 messages/second. The rate is set by Apple
 * Events (one or more per property, per message), not by the database, so it cannot be
 * tuned from our side. A busy Gmail account compounds it: penfield33@gmail.com holds
 * 23,271 messages in Sent Mail, which is ~70 minutes to read on its own, and a full
 * pull across all three accounts is a multi-hour background job.
 *
 * The window is applied WHILE READING, so an out-of-window message costs one Apple
 * Event (its date) instead of the full record - reading the last 90 days of that same
 * Sent Mail took 311 reads instead of 23,271. That is why the default is narrow: it
 * turns an hour-long background job into a few minutes.
 *
 * MAIL_SYNC_SINCE  a relative window (90d, 24mo, 3w, 2y), an ISO date, or "all".
 *                  Defaults to 90 days. "all" reads complete history - deliberate,
 *                  slow, and normally something to run in the background.
 * MAIL_SYNC_MAX_PER_MAILBOX  hard cap per mailbox per run, newest first. Off by
 *                  default (0/off). This, not the window, is what bounds a mailbox
 *                  whose recent traffic alone is large.
 *
 * Both are read-only bounds: nothing is deleted, and a later run with a wider window
 * simply lands the older messages (landing is replay-safe on message identity).
 */
function mailSyncWindow(): MailWindow {
  const raw = (process.env.MAIL_SYNC_SINCE ?? DEFAULT_MAIL_SYNC_SINCE).trim().toLowerCase()
  let since: string | null = null
  if (raw && raw !== 'all' && raw !== 'none' && raw !== 'off') {
    const relative = raw.match(/^(\d+)(d|w|mo|y)$/)
    if (relative) {
      const count = Number(relative[1])
      const unit = relative[2]
      const days = unit === 'd' ? count : unit === 'w' ? count * 7 : unit === 'mo' ? count * 30 : count * 365
      if (count <= 0) throw new Error(`MAIL_SYNC_SINCE must be a positive window (got "${raw}").`)
      since = new Date(Date.now() - days * 86_400_000).toISOString()
    } else {
      const parsed = new Date(raw)
      if (Number.isNaN(parsed.getTime())) {
        throw new Error(`MAIL_SYNC_SINCE must be an ISO date, a relative window like 24mo/90d/2y, or "all" (got "${raw}").`)
      }
      since = parsed.toISOString()
    }
  }

  const capRaw = process.env.MAIL_SYNC_MAX_PER_MAILBOX?.trim().toLowerCase()
  let maxPerMailbox: number | null = null
  if (capRaw && capRaw !== '0' && capRaw !== 'off') {
    const cap = Number(capRaw)
    if (!Number.isInteger(cap) || cap <= 0) {
      throw new Error(`MAIL_SYNC_MAX_PER_MAILBOX must be a positive integer, or 0/off (got "${capRaw}").`)
    }
    maxPerMailbox = cap
  }

  return { since, maxPerMailbox }
}

async function acquireMetadata(
  execute: QueryExecutor,
  verifyOnly = false,
): Promise<ICloudMailObservation[]> {
  const accounts = mailAppAccounts()
  const window = mailSyncWindow()
  console.log(
    window.since
      ? `Apple Mail window: since ${window.since}${window.maxPerMailbox ? `, max ${window.maxPerMailbox} per mailbox` : ''}`
      : 'Apple Mail window: ALL history (MAIL_SYNC_SINCE=all)',
  )
  const internal = internalAddresses()
  for (const candidate of accounts) {
    if (!internal.has(candidate)) {
      throw new Error(`Mail.app account ${candidate} must be listed in EMAIL_INTERNAL_ADDRESSES.`)
    }
  }

  // Dedupe and classification counters span EVERY synced account: one message can
  // surface in two mailboxes (one account's inbox, another's sent) and must collapse
  // to a single observation.
  const seen = new Set<string>()
  const observations: ICloudMailObservation[] = []
  let ambiguous = 0
  let invalid = 0
  let landedTotal = 0
  let replayedTotal = 0
  let unlandableTotal = 0
  let exportedTotal = 0

  for (const account of accounts) {
    console.log(`reading Apple Mail metadata through authenticated Mail.app account=${account}`)
    const records = await runMailExporter(account, window)
    exportedTotal += records.length
    if (verifyOnly) {
      console.log(`Apple Mail.app account access verified: ${account}`)
      continue
    }

  // LANDING — before ANY identity decision. Every exported source record lands in
  // l_applemail here, including the ones the classification below will skip
  // (internal-only, ambiguous, no timestamp). Those are still source evidence and
  // must not be silently lost. No Person decision, no CRM decision, no promotion.
  let landed = 0
  let replayed = 0
  let unlandable = 0
  for (const record of records) {
    const sourceMessageId = appleMailReplayId({
      messageId: record.messageId,
      mailboxKind: record.mailbox,
      localId: record.localId,
    })
    if (!sourceMessageId) {
      // No stable identity means it cannot be replayed safely. Recorded, never
      // invented - a random id would duplicate the row on every run.
      unlandable += 1
      continue
    }
    const inserted = await landAppleMail(
      {
        sourceAccount: account,
        sourceMessageId,
        mailboxKind: record.mailbox,
        mailboxName: record.mailboxName,
        localId: record.localId,
        messageId: record.messageId,
        occurredAt: record.occurredAt ? new Date(record.occurredAt).toISOString() : null,
        sender: record.sender,
        toRecipients: record.to,
        ccRecipients: record.cc,
        bccRecipients: record.bcc,
        subject: record.subject,
        // The UNMODIFIED exporter record - not a normalized evidence object.
        raw: record,
      },
      execute,
    )
    if (inserted) landed += 1
    else replayed += 1
  }
  console.log(
    `l_applemail[${account}]: ${landed} landed, ${replayed} replayed, ${unlandable} without a stable id (of ${records.length} exported)`,
  )
  landedTotal += landed
  replayedTotal += replayed
  unlandableTotal += unlandable
  for (let index = 0; index < records.length; index += 1) {
    const record = records[index]
    const sender = senderAddress(record.sender)
    let direction: 'inbound' | 'outbound'
    let externalEmail: string
    let displayName: string | null

    if (record.mailbox === 'inbox') {
      if (!sender?.address || internal.has(sender.address)) {
        invalid += 1
        continue
      }
      direction = 'inbound'
      externalEmail = sender.address
      displayName = sender.name
    } else {
      const recipients = [...record.to, ...record.cc, ...record.bcc]
      const external = new Map<string, string | null>()
      for (const recipient of recipients) {
        const address = recipient.address ? normalizeMailbox(recipient.address) : null
        if (address && !internal.has(address) && !external.has(address)) {
          external.set(address, recipient.name?.trim() || null)
        }
      }
      if (external.size !== 1) {
        ambiguous += 1
        continue
      }
      direction = 'outbound'
      ;[externalEmail, displayName] = external.entries().next().value!
    }

    if (!record.occurredAt) {
      invalid += 1
      continue
    }
    const sourceExternalId = record.messageId?.trim()
      ? `message-id:${record.messageId.trim()}`
      : `mail-local:${record.mailbox}:${record.localId}`
    if (seen.has(sourceExternalId)) continue
    seen.add(sourceExternalId)
    observations.push({
      sourceExternalId,
      sourceAccount: account,
      mailbox: record.mailboxName,
      uid: record.localId,
      uidValidity: 'apple-mail-local',
      occurredAt: new Date(record.occurredAt).toISOString(),
      direction,
      externalEmail,
      displayName,
      subject: boundedEmailSubject(record.subject ?? undefined),
    })

    const processed = index + 1
    if (processed % 100 === 0 || processed === records.length) {
      console.log(`normalize progress: ${processed}/${records.length} | direct=${observations.length} ambiguous=${ambiguous} invalid=${invalid}`)
    }
  }
  }

  console.log(
    `l_applemail total: ${landedTotal} landed, ${replayedTotal} replayed, ${unlandableTotal} without a stable id (of ${exportedTotal} exported across ${accounts.length} account(s))`,
  )
  console.log(`metadata acquisition complete: ${observations.length} direct messages | ambiguous=${ambiguous} invalid=${invalid}`)
  return observations
}

async function refresh(execute: QueryExecutor) {
  await execute`refresh materialized view concurrently mv_client_relationship_channels`
  await execute`refresh materialized view concurrently mv_client_directory`
  await execute`refresh materialized view concurrently mv_client_contact_history`
}

export async function intakeMetadata(
  target: EnvTarget,
  observations: ICloudMailObservation[],
  execute: QueryExecutor,
) {
  console.log(`=== iCloud Mail metadata intake (${target.toUpperCase()}) ===`)
  const builds = buildICloudMailEvidence(observations)
  const { lookup } = await createInMemoryPersonLookup(execute)
  const tally: Record<string, number> = {}
  for (let index = 0; index < builds.length; index += 1) {
    const build = builds[index]
    const id = await upsertRelationshipEvidence(build.evidence, build.fingerprint, undefined, execute)
    const decision = await reconcileEvidence(build.evidence, lookup)
    await recordReconcileDecision(id, decision, execute)
    tally[decision.reviewState] = (tally[decision.reviewState] ?? 0) + 1
    const processed = index + 1
    if (processed % 25 === 0 || processed === builds.length) {
      console.log(`reconcile progress: ${processed}/${builds.length} identities`)
    }
  }
  console.log('reconcile tally:', JSON.stringify(tally))

  // Derived from the observations themselves, so every synced Mail.app account is
  // honoured instead of only the single legacy ICLOUD_MAIL_ADDRESS.
  const syncedAccounts = new Set(observations.map((observation) => observation.sourceAccount))
  const linked = new Map(
    (await getRelationshipEvidenceRows(ICLOUD_MAIL_SOURCE, execute))
      .filter((row) => syncedAccounts.has(row.sourceAccount))
      .filter((row) => row.reviewState === 'exact_linked' && row.canonicalPersonId)
      .map((row) => [row.sourceIdentityKey, row.canonicalPersonId!]),
  )
  let inserted = 0
  let replayed = 0
  let skippedUnlinked = 0
  for (let index = 0; index < observations.length; index += 1) {
    const observation = observations[index]
    const personId = linked.get(observation.externalEmail)
    if (!personId) {
      skippedUnlinked += 1
    } else {
      const result = await createInteraction(observationToInteraction(observation, personId), execute)
      if (result.created) inserted += 1
      else replayed += 1
    }
    const processed = index + 1
    if (processed % 100 === 0 || processed === observations.length) {
      console.log(`materialize progress: ${processed}/${observations.length} | inserted=${inserted} replayed=${replayed} unlinked=${skippedUnlinked}`)
    }
  }

  await refresh(execute)
  console.log(`intake complete: identities=${builds.length} inserted=${inserted} replayed=${replayed} unlinked=${skippedUnlinked}`)
  return { identities: builds.length, inserted, replayed, skippedUnlinked, tally }
}

async function main() {
  const target = process.argv[2]
  if (target !== 'dev' && target !== 'prod') {
    throw new Error('Usage: icloud-mail-sync.ts <dev|prod>')
  }
  const verifyOnly = process.argv.includes('--verify-only')
  // The pool is created BEFORE acquisition so landing can write source evidence as
  // the exporter's records arrive - landing must precede classification.
  const pool = createPoolExecutor(targetDatabaseUrl(target))
  try {
    const observations = await acquireMetadata(pool.execute, verifyOnly)
    if (verifyOnly) return
    await intakeMetadata(target, observations, pool.execute)
  } finally {
    await pool.end()
  }
}

if (process.argv[1]?.endsWith('icloud-mail-sync.ts')) {
  main().catch((error) => {
    console.error(`Apple Mail metadata sync failed: ${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  })
}
