# Apple Mail intake - code review bundle

Generated: 2026-09-11T21:17:06Z

## Symptom under review

The exporter reads Mail.app metadata (never bodies) through an osascript JXA bridge.
On penfield33@gmail.com the INBOX cannot be read by Mail.app scripting at all, while other
mailboxes on the same account read fine. Observed via a standalone probe, with no window or
cap logic involved:

  - mailbox.messages().length            -> Error: An error occurred.
  - mailbox.messages()[0].subject()      -> Error: An error occurred.   (note: index 0, the first message)
  - whose(dateReceived > 90d).length     -> AppleEvent timed out
  - whose(dateReceived > 7d).length      -> AppleEvent timed out (-1712) after 120s

Control, same Mac, same mechanism, different mailbox:

  - culebraluxe@gmail.com INBOX (686 messages): count, index 0, and whose(90d) all work (594).
  - penfield33@gmail.com Important (43,575 messages): enumerates and counts fine.

So the failure is not permissions (the account authenticates), and not a size cutoff (a 43k
mailbox reads fine). Reading even message index 0 fails, which no date window can influence.
Question for review: what in this code could produce that mailbox-level failure, and could the
three interrupted runs have left Mail.app in a wedged state rather than the mailbox being damaged?

## 1/5 scripts/macbridge/apple-mail-metadata.jxa

```javascript
ObjC.import('Foundation')

function safe(read, fallback = null) {
  try {
    const value = read()
    return value === undefined || value === null ? fallback : value
  } catch (_) {
    return fallback
  }
}

function normalized(value) {
  return String(value || '').trim().toLowerCase()
}

function recipientsOf(message, property) {
  const recipients = safe(() => message[property](), [])
  return recipients.map((recipient) => ({
    address: safe(() => recipient.address(), null),
    name: safe(() => recipient.name(), null),
  }))
}

function write(handle, value) {
  const data = $(value).dataUsingEncoding($.NSUTF8StringEncoding)
  handle.writeData(data)
}

let outputHandle = null

function out(value) {
  if (!outputHandle) throw new Error('Mail metadata output file is not open')
  write(outputHandle, value)
}

function progress(value) {
  write($.NSFileHandle.fileHandleWithStandardError, value + '\n')
}

// Mail.app scripting fails to enumerate some mailboxes outright - on this Mac
// penfield33@gmail.com's INBOX and Gmail's virtual "All Mail" both throw before any
// message is read, while the same account's Sent Mail enumerates fine. That is a
// Mail.app limitation, not a window/cap artefact, so an unreadable mailbox is
// SKIPPED and named rather than allowed to abort every other account's pull.
function openMailbox(mailbox) {
  try {
    const messages = mailbox.messages()
    const total = messages.length
    if (typeof total !== 'number') return null
    return { messages, total }
  } catch (_) {
    return null
  }
}

function run(argv) {
  const targetAddress = normalized(argv[0])
  const outputPath = String(argv[1] || '')
  if (!targetAddress || !outputPath) {
    throw new Error('Usage: osascript -l JavaScript apple-mail-metadata.jxa <email> <output-jsonl> [since-iso|all] [max-per-mailbox]')
  }
  outputHandle = $.NSFileHandle.fileHandleForWritingAtPath($(outputPath))
  if (!outputHandle) throw new Error('Unable to open Mail metadata output file')

  const Mail = Application('Mail')
  const accounts = Mail.accounts()
  const account = accounts.find((candidate) =>
    safe(() => candidate.emailAddresses(), []).map(normalized).includes(targetAddress),
  )
  if (!account) {
    const available = accounts.flatMap((candidate) => safe(() => candidate.emailAddresses(), []))
    throw new Error(
      'Mail.app has no enabled account for ' + targetAddress +
      '. Available account addresses: ' + available.join(', '),
    )
  }

  const selected = []
  for (const mailbox of account.mailboxes()) {
    const name = normalized(safe(() => mailbox.name(), ''))
    if (name === 'inbox') selected.push({ kind: 'inbox', mailbox })
    if (['sent', 'sent messages', 'sent mail'].includes(name)) {
      selected.push({ kind: 'sent', mailbox })
    }
  }

  if (!selected.some((entry) => entry.kind === 'inbox')) {
    throw new Error('Mail.app account did not expose an Inbox mailbox')
  }
  if (!selected.some((entry) => entry.kind === 'sent')) {
    throw new Error('Mail.app account did not expose a Sent mailbox')
  }

  const sinceArg = normalized(argv[2])
  let cutoff = null
  if (sinceArg && sinceArg !== 'all') {
    const parsedSince = new Date(String(argv[2]))
    if (isNaN(parsedSince.getTime())) {
      throw new Error('Invalid since date: ' + String(argv[2]))
    }
    cutoff = parsedSince.getTime()
  }
  const capArg = Number(argv[3] || 0)
  const maxPerMailbox = Number.isFinite(capArg) && capArg > 0 ? Math.floor(capArg) : 0

  // Consecutive out-of-window messages that end a mailbox scan. Mail.app lists a
  // mailbox newest-first, so once this far past the cutoff there is nothing left to
  // find. The break switches off the moment the mailbox proves NOT to be
  // date-descending, so an oddly-sorted mailbox is scanned in full rather than
  // silently truncated.
  const STALE_LIMIT = 250

  let exported = 0
  let outsideWindow = 0
  const skippedMailboxes = []
  for (const { kind, mailbox } of selected) {
    const name = safe(() => mailbox.name(), kind)
    const opened = openMailbox(mailbox)
    if (!opened) {
      progress('WARNING mailbox ' + name + ': unreadable through Mail.app scripting - SKIPPED')
      skippedMailboxes.push(name)
      continue
    }
    const messages = opened.messages
    const total = opened.total
    progress('mailbox ' + name + ': ' + total + ' messages')
    if (cutoff) progress('mailbox ' + name + ': window since ' + new Date(cutoff).toISOString())

    let staleRun = 0
    let descending = true
    let previous = null
    let emittedHere = 0
    let stoppedEarly = false

    for (let index = 0; index < total; index += 1) {
      const message = messages[index]
      try {
        const occurred = kind === 'sent'
          ? safe(() => message.dateSent(), null)
          : safe(() => message.dateReceived(), null)
        const occurredMs = occurred instanceof Date ? occurred.getTime() : null

        // Newer-than-previous means the mailbox is not date-descending; disable the
        // early break permanently so we scan everything rather than lose records.
        if (previous !== null && occurredMs !== null && occurredMs > previous) descending = false
        if (occurredMs !== null) previous = occurredMs

        // A message with no readable date cannot be windowed, so it is kept.
        const inWindow = !cutoff || occurredMs === null || occurredMs >= cutoff
        if (inWindow) {
          staleRun = 0
        } else {
          outsideWindow += 1
          staleRun += 1
          if (descending && staleRun >= STALE_LIMIT) {
            stoppedEarly = true
            progress('mailbox ' + name + ': stopping early - ' + staleRun + ' consecutive messages older than the window')
            break
          }
        }

        if (inWindow) {
          if (maxPerMailbox && emittedHere >= maxPerMailbox) {
            stoppedEarly = true
            progress('mailbox ' + name + ': stopping early - cap of ' + maxPerMailbox + ' reached')
            break
          }
          const record = {
          mailbox: kind,
          mailboxName: name,
          localId: Number(safe(() => message.id(), index + 1)),
          messageId: safe(() => message.messageId(), null),
          occurredAt: occurred instanceof Date ? occurred.toISOString() : null,
          sender: safe(() => message.sender(), null),
          to: recipientsOf(message, 'toRecipients'),
          cc: recipientsOf(message, 'ccRecipients'),
          bcc: recipientsOf(message, 'bccRecipients'),
          subject: safe(() => message.subject(), null),
        }
          out(JSON.stringify(record) + '\n')
          emittedHere += 1
          exported += 1
        }
      } catch (error) {
        progress('skipped unreadable Mail.app message ' + (index + 1) + ': ' + String(error))
      }

      const processed = index + 1
      if (processed % 100 === 0 || processed === total) {
        progress('export progress ' + name + ': ' + processed + '/' + total + ' messages')
      }
    }

    progress(
      'mailbox ' + name + ': exported ' + emittedHere +
      (stoppedEarly ? ' (stopped early, ' + total + ' available)' : ' (scanned ' + total + ')'),
    )
  }

  progress(
    'Mail.app metadata export complete: ' + exported + ' messages' +
    (outsideWindow ? ', ' + outsideWindow + ' outside the window' : ''),
  )
  if (skippedMailboxes.length) {
    progress('WARNING skipped unreadable mailboxes: ' + skippedMailboxes.join(', '))
  }
}

```

## 2/5 scripts/icloud-mail-sync.ts

```typescript
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

```

## 3/5 scripts/email-sync.sh

```bash
#!/usr/bin/env bash
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SELF_DIR/.." && pwd -P)"
cd "$REPO_ROOT"

now() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }
log() { echo "[email-sync $(now)] $*"; }
fail() { log "ERROR: $*" >&2; exit 1; }

log "verifying metadata-only authenticated Mail.app environment"
[ -f .env.local ] || fail "missing .env.local"
[ -d node_modules/tsx ] || fail "dependencies missing; run pnpm install"
command -v osascript >/dev/null 2>&1 || fail "osascript is required; run this sync on Lisa's Mac"

node -e '
const fs=require("fs");
const text=fs.readFileSync(".env.local","utf8");
const values=new Map(text.split(/\r?\n/).map(l=>l.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)).filter(Boolean).map(m=>[m[1],m[2].trim()]));
const required=["DATABASE_URL_DEV","DATABASE_URL_PROD","ICLOUD_MAIL_ADDRESS","EMAIL_INTERNAL_ADDRESSES"];
const missing=required.filter(k=>!values.get(k));
if(missing.length){console.error("missing/empty env keys: "+missing.join(", "));process.exit(2)}
' || fail "environment is incomplete"

log "PROD Apple Mail metadata sync start"
node --env-file=.env.local --import tsx scripts/icloud-mail-sync.ts prod
log "PROD Apple Mail metadata sync success"
log "sync complete"
```

## 4/5 db/landing.ts (from appleMailReplayId to end of file)

```typescript
export function appleMailReplayId(input: {
  messageId?: string | null
  mailboxKind?: string | null
  localId?: number | null
}): string | null {
  const messageId = (input.messageId ?? '').trim()
  if (messageId) return `message-id:${messageId}`
  if (input.mailboxKind && typeof input.localId === 'number') {
    return `mail-local:${input.mailboxKind}:${input.localId}`
  }
  return null
}

export async function landAppleMail(
  input: LandedAppleMail,
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_applemail (
      source_account, source_message_id, mailbox_kind, mailbox_name, local_id,
      message_id, occurred_at, sender, to_recipients, cc_recipients,
      bcc_recipients, subject, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.mailboxKind ?? null},
      ${input.mailboxName ?? null}, ${input.localId ?? null}, ${input.messageId ?? null},
      ${input.occurredAt ?? null}::timestamptz, ${input.sender ?? null},
      ${input.toRecipients == null ? null : JSON.stringify(input.toRecipients)}::jsonb,
      ${input.ccRecipients == null ? null : JSON.stringify(input.ccRecipients)}::jsonb,
      ${input.bccRecipients == null ? null : JSON.stringify(input.bccRecipients)}::jsonb,
      ${input.subject ?? null}, ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) && rows.length > 0
}

export type LandedCall = {
  sourceAccount: string | null
  sourceMessageId: string
  handle?: string | null
  /** 'incoming' | 'outgoing' */
  direction?: string | null
  /** 'audio' | 'video' — video is FaceTime. */
  callType?: string | null
  answered?: boolean | null
  durationSeconds?: number | null
  startedAt?: string | null
  raw: unknown
}

export async function landCall(input: LandedCall, execute?: QueryExecutor): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_call (
      source_account, source_message_id, handle, direction, call_type,
      answered, duration_seconds, started_at, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.handle ?? null},
      ${input.direction ?? null}, ${input.callType ?? null}, ${input.answered ?? null},
      ${input.durationSeconds ?? null}, ${input.startedAt ?? null}::timestamptz,
      ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) && rows.length > 0
}

export type LandedWhatsapp = {
  sourceAccount: string | null
  sourceMessageId: string
  conversationId?: string | null
  fromAddress?: string | null
  toAddress?: string | null
  direction?: string | null
  messageType?: string | null
  text?: string | null
  mediaId?: string | null
  sentAt?: string | null
  raw: unknown
}

export async function landWhatsapp(
  input: LandedWhatsapp,
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_whatsapp (
      source_account, source_message_id, conversation_id, from_address, to_address,
      direction, message_type, text_content, media_id, sent_at, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.conversationId ?? null},
      ${input.fromAddress ?? null}, ${input.toAddress ?? null}, ${input.direction ?? null},
      ${input.messageType ?? null}, ${input.text ?? null}, ${input.mediaId ?? null},
      ${input.sentAt ?? null}::timestamptz, ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) && rows.length > 0
}

export type LandedCalendarEvent = {
  sourceAccount: string | null
  sourceMessageId: string
  title?: string | null
  startsAt?: string | null
  endsAt?: string | null
  allDay?: boolean | null
  location?: string | null
  organizer?: string | null
  attendees?: unknown
  raw: unknown
}

export async function landCalendarEvent(
  input: LandedCalendarEvent,
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_calendar (
      source_account, source_message_id, title, starts_at, ends_at,
      all_day, location, organizer, attendees, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.title ?? null},
      ${input.startsAt ?? null}::timestamptz, ${input.endsAt ?? null}::timestamptz,
      ${input.allDay ?? null}, ${input.location ?? null}, ${input.organizer ?? null},
      ${input.attendees == null ? null : JSON.stringify(input.attendees)}::jsonb,
      ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) && rows.length > 0
}
```

## 5/5 effective configuration (secrets excluded)

```
ICLOUD_MAIL_ADDRESS=lisa@culebraluxe.com
EMAIL_INTERNAL_ADDRESSES=penfield33@gmail.com,culebraluxe@gmail.com,lisa@culebraluxe.com
MAIL_APP_ACCOUNTS=penfield33@gmail.com,culebraluxe@gmail.com,lisa@culebraluxe.com
MAIL_SYNC_SINCE=90d
MAIL_SYNC_MAX_PER_MAILBOX=400
```
