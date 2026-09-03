// Apple-hosted iCloud Mail metadata intake.
// Reads only IMAP ENVELOPE metadata: participants, timestamp, subject, and
// Message-ID/UID provenance. It never requests bodies, snippets, attachments,
// or raw MIME and performs no mailbox mutations.
import { ImapFlow, type FetchMessageObject, type ListResponse } from 'imapflow'
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
  boundedEmailSubject,
  buildICloudMailEvidence,
  classifyEnvelope,
  ICLOUD_MAIL_SOURCE,
  normalizeMailbox,
  observationToInteraction,
  type ICloudMailObservation,
} from '../lib/relationship-intel/icloud-mail'

type EnvTarget = 'dev' | 'prod'

type ImapError = Error & {
  code?: string
  response?: string
  responseText?: string
  serverResponseCode?: string
  executedCommand?: string
}

function describeImapError(error: unknown): string {
  if (!(error instanceof Error)) return String(error)
  const imapError = error as ImapError
  const details = [
    imapError.message,
    imapError.code ? `code=${imapError.code}` : null,
    imapError.serverResponseCode ? `server=${imapError.serverResponseCode}` : null,
    imapError.executedCommand
      ? `command=${imapError.executedCommand.trim().split(/\\s+/).slice(0, 2).join(' ')}`
      : null,
    imapError.responseText ? `response=${imapError.responseText}` : null,
    imapError.response ? `raw=${imapError.response}` : null,
  ].filter((value): value is string => Boolean(value))
  return [...new Set(details)].join(' | ')
}

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

function sourceId(message: FetchMessageObject, mailbox: string, uidValidity: string): string {
  const messageId = message.envelope?.messageId?.trim()
  return messageId ? `message-id:${messageId}` : `imap-uid:${mailbox}:${uidValidity}:${message.uid}`
}

async function scanMailbox(
  client: ImapFlow,
  mailbox: ListResponse,
  account: string,
  internal: ReadonlySet<string>,
  seen: Set<string>,
): Promise<ICloudMailObservation[]> {
  const lock = await client.getMailboxLock(mailbox.path, { readOnly: true })
  try {
    const exists = client.mailbox && typeof client.mailbox === 'object' ? client.mailbox.exists : 0
    const uidValidity = client.mailbox && typeof client.mailbox === 'object'
      ? client.mailbox.uidValidity.toString()
      : 'unknown'
    console.log(`mailbox ${mailbox.path}: ${exists} messages`)
    if (!exists) return []

    const observations: ICloudMailObservation[] = []
    let scanned = 0
    for await (const message of client.fetch('1:*', { envelope: true, internalDate: true })) {
      scanned += 1
      const envelope = message.envelope
      const classified = envelope ? classifyEnvelope(envelope, internal) : { ok: false as const, reason: 'unrelated' as const }
      if (classified.ok) {
        const occurred = envelope?.date ?? message.internalDate
        const occurredAt = occurred ? new Date(occurred).toISOString() : null
        const id = sourceId(message, mailbox.path, uidValidity)
        if (occurredAt && !seen.has(id)) {
          seen.add(id)
          observations.push({
            sourceExternalId: id,
            sourceAccount: account,
            mailbox: mailbox.path,
            uid: message.uid,
            uidValidity,
            occurredAt,
            direction: classified.direction,
            externalEmail: classified.externalEmail,
            displayName: classified.displayName,
            subject: boundedEmailSubject(envelope?.subject),
          })
        }
      }
      if (scanned % 100 === 0 || scanned === exists) {
        console.log(`scan progress ${mailbox.path}: ${scanned}/${exists} messages | direct=${observations.length}`)
      }
    }
    return observations
  } finally {
    lock.release()
  }
}

async function acquireMetadata(verifyOnly = false): Promise<ICloudMailObservation[]> {
  const account = requiredEnv('ICLOUD_MAIL_ADDRESS').toLowerCase()
  const internal = internalAddresses()
  if (!internal.has(account)) throw new Error('ICLOUD_MAIL_ADDRESS must be listed in EMAIL_INTERNAL_ADDRESSES.')

  const client = new ImapFlow({
    host: process.env.ICLOUD_MAIL_IMAP_HOST?.trim() || 'imap.mail.me.com',
    port: 993,
    secure: true,
    auth: {
      user: requiredEnv('ICLOUD_MAIL_USERNAME'),
      pass: requiredEnv('ICLOUD_MAIL_APP_PASSWORD'),
      loginMethod: 'LOGIN',
    },
    logger: false,
  })
  client.on('error', (error) => console.error(`iCloud IMAP error: ${describeImapError(error)}`))

  try {
    console.log(`connecting to Apple iCloud Mail address=${account} username=${requiredEnv('ICLOUD_MAIL_USERNAME')} method=LOGIN`)
    await client.connect()
    if (verifyOnly) {
      console.log('Apple iCloud Mail authentication verified')
      return []
    }
    const mailboxes = await client.list()
    const selected = mailboxes.filter(
      (mailbox) => mailbox.path.toUpperCase() === 'INBOX' || mailbox.specialUse === '\\Sent',
    )
    if (!selected.some((mailbox) => mailbox.path.toUpperCase() === 'INBOX')) {
      throw new Error('Apple IMAP did not expose INBOX.')
    }
    if (!selected.some((mailbox) => mailbox.specialUse === '\\Sent')) {
      throw new Error('Apple IMAP did not expose a Sent mailbox.')
    }
    const seen = new Set<string>()
    const observations: ICloudMailObservation[] = []
    for (const mailbox of selected) {
      observations.push(...(await scanMailbox(client, mailbox, account, internal, seen)))
    }
    console.log(`metadata acquisition complete: ${observations.length} direct messages`)
    return observations
  } finally {
    if (client.usable) await client.logout().catch(() => undefined)
  }
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

  const linked = new Map(
    (await getRelationshipEvidenceRows(ICLOUD_MAIL_SOURCE, execute))
      .filter((row) => row.sourceAccount === requiredEnv('ICLOUD_MAIL_ADDRESS').toLowerCase())
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
  const observations = await acquireMetadata(verifyOnly)
  if (verifyOnly) return
  const pool = createPoolExecutor(targetDatabaseUrl(target))
  try {
    await intakeMetadata(target, observations, pool.execute)
  } finally {
    await pool.end()
  }
}

if (process.argv[1]?.endsWith('icloud-mail-sync.ts')) {
  main().catch((error) => {
    console.error(`iCloud Mail sync failed: ${describeImapError(error)}`)
    process.exitCode = 1
  })
}
