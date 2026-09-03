// Apple-hosted iCloud Mail metadata intake.
// Reads only authenticated Mail.app metadata: participants, timestamp, subject,
// and Message-ID/local provenance. It never requests bodies, snippets, attachments,
// or raw MIME and performs no mailbox mutations.
import { spawn } from 'node:child_process'
import { resolve } from 'node:path'
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

function runMailExporter(account: string): Promise<LocalMailRecord[]> {
  return new Promise((resolveRecords, reject) => {
    const exporter = resolve(process.cwd(), 'scripts/macbridge/apple-mail-metadata.jxa')
    const child = spawn('/usr/bin/osascript', ['-l', 'JavaScript', exporter, account], {
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let stdout = ''
    let stderr = ''
    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stdout.on('data', (chunk: string) => {
      stdout += chunk
    })
    child.stderr.on('data', (chunk: string) => {
      stderr += chunk
      process.stderr.write(chunk)
    })
    child.on('error', reject)
    child.on('close', (code) => {
      if (code !== 0) {
        reject(new Error(`Mail.app metadata exporter failed (exit ${code}): ${stderr.trim()}`))
        return
      }
      try {
        const records = stdout
          .split(/\\r?\\n/)
          .filter(Boolean)
          .map((line, index) => {
            try {
              return JSON.parse(line) as LocalMailRecord
            } catch {
              throw new Error(`Invalid Mail.app metadata JSON on line ${index + 1}`)
            }
          })
        resolveRecords(records)
      } catch (error) {
        reject(error)
      }
    })
  })
}

async function acquireMetadata(verifyOnly = false): Promise<ICloudMailObservation[]> {
  const account = requiredEnv('ICLOUD_MAIL_ADDRESS').toLowerCase()
  const internal = internalAddresses()
  if (!internal.has(account)) throw new Error('ICLOUD_MAIL_ADDRESS must be listed in EMAIL_INTERNAL_ADDRESSES.')

  console.log(`reading Apple Mail metadata through authenticated Mail.app account=${account}`)
  const records = await runMailExporter(account)
  if (verifyOnly) {
    console.log('Apple Mail.app account access verified')
    return []
  }

  const seen = new Set<string>()
  const observations: ICloudMailObservation[] = []
  let ambiguous = 0
  let invalid = 0
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
