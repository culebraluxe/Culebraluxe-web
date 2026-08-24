#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Apple Contacts — ODS client-priming loader (V1).
//
// Reads a validated Apple Contacts export, lowers each contact through the
// canonical intake projection into the durable integration inbox, and persists
// the complete neutral staged-contact profile as an immutable ODS revision.
//
// Dirty data belongs in ODS STAGING. This loader does NOT promote contacts:
// it creates no Person, Client, interaction, Deal, workflow, task, event, or
// outbox row.
//
// Identity: sourceSystem + sourceAccount + sourceId (CNContact.identifier).
// The batch/export id is provenance only — never contact identity.
//
// Replay / revision behavior:
//   same identity + same fingerprint      -> exact replay, no new revision
//   same identity + different fingerprint -> next immutable revision (supersedes)
//   new identity                          -> revision 1
//   same batch id + same checksum         -> safe replay
//   same batch id + different checksum    -> truthful conflict (fail closed)
//
// The loader is idempotent / resumable (constraint-backed dedup).
// ---------------------------------------------------------------------------
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { Pool } from '@neondatabase/serverless'
import {
  APPLE_CONTACTS_ADAPTER,
  APPLE_CONTACTS_ADAPTER_VERSION,
  APPLE_CONTACTS_SOURCE_SYSTEM,
  appleContactToBatchItem,
  parseAppleContactExportBatch,
  type AppleContactExport,
  type AppleContactExportBatch,
} from '../lib/intake/apple-contacts'
import { lowerBatchItemToIntakeMessage } from '../lib/intake/batch'
import { insertOrReadIntegrationInbox } from '../db/integration-inbox'
import type { QueryExecutor } from '../db/query-executor'
import { fileURLToPath } from 'node:url'
import { resolve as resolvePath } from 'node:path'

const isMain = process.argv[1]
  ? fileURLToPath(import.meta.url) === resolvePath(process.argv[1])
  : false

function flag(name: string): string | undefined {
  const i = process.argv.indexOf(name)
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : undefined
}
const file = flag('--file')
const sourceAccount = flag('--source-account')
const env = (flag('--env') ?? 'dev').toLowerCase()

async function runMain() {
  const url = env === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  if (!file) {
    console.error('--file <export.json> is required')
    process.exit(2)
  }
  if (!sourceAccount) {
    console.error('--source-account is required (fail closed on empty)')
    process.exit(2)
  }
  if (env !== 'dev' && env !== 'prod') {
    console.error('--env must be dev|prod')
    process.exit(2)
  }
  if (!url) {
    console.error(`No DATABASE_URL_${env.toUpperCase()} configured (fail closed; generic DATABASE_URL is never used here)`)
    process.exit(2)
  }
  if (env === 'prod' && url === process.env.DATABASE_URL_DEV) {
    console.error('PROD load selected but the configured connection is the DEV URL (fail closed)')
    process.exit(2)
  }
  const pool = new Pool({ connectionString: url, ssl: true })
  const q = makeExecutor(pool)
  try {
    await main(q)
  } finally {
    await pool.end()
  }
}
if (isMain) {
  runMain().catch((err) => {
    console.error((err as Error).message ?? String(err))
    process.exit(1)
  })
}

/** Tagged-template QueryExecutor over the Neon WebSocket pool. */
function makeExecutor(pool: Pool): QueryExecutor {
  return ((strings, ...params) => {
    const text = strings.reduce(
      (acc, s, i) => acc + s + (i < params.length ? `$${i + 1}` : ''),
      '',
    )
    return pool.query({ text, values: params }).then((r) => r.rows as never[])
  }) as QueryExecutor
}

/** Deterministic normalized profile (trimmed, stable ordering). */
export function normalizeProfile(contact: AppleContactExport): Record<string, unknown> {
  const labelValue = (items: { sourceLabel: string | null; value: string }[]) =>
    [...items]
      .map((i) => ({ label: (i.sourceLabel ?? '').trim(), value: i.value.trim() }))
      .filter((i) => i.value !== '')
      .sort((a, b) => `${a.label}|${a.value}`.localeCompare(`${b.label}|${b.value}`))
  return {
    name: {
      prefix: contact.namePrefix.trim(),
      given: contact.givenName.trim(),
      middle: contact.middleName.trim(),
      family: contact.familyName.trim(),
      suffix: contact.nameSuffix.trim(),
      nickname: contact.nickname.trim(),
    },
    organization: contact.organization.trim(),
    department: contact.department.trim(),
    jobTitle: contact.jobTitle.trim(),
    emails: labelValue(contact.emails),
    phones: labelValue(contact.phones),
    postalAddresses: [...contact.postalAddresses]
      .map((a) => ({
        label: (a.sourceLabel ?? '').trim(),
        street: a.street.trim(),
        city: a.city.trim(),
        state: a.state.trim(),
        postalCode: a.postalCode.trim(),
        country: a.country.trim(),
        isoCountryCode: a.isoCountryCode.trim(),
      }))
      .sort((a, b) =>
        `${a.label}|${a.street}|${a.city}`.localeCompare(`${b.label}|${b.street}|${b.city}`),
      ),
  }
}

export function profileFingerprint(profile: Record<string, unknown>): string {
  return createHash('sha256').update(JSON.stringify(profile)).digest('hex')
}

/**
 * Pure staging decision for one contact identity:
 *   no prior revision  -> 'new'
 *   same fingerprint   -> 'replay' (exact replay, no new revision)
 *   different fingerprint -> 'changed' (next immutable revision supersedes)
 */
export function decideStagingOutcome(
  prior: { payload_fingerprint: string } | undefined,
  fingerprint: string,
): 'new' | 'replay' | 'changed' {
  if (!prior) return 'new'
  if (prior.payload_fingerprint === fingerprint) return 'replay'
  return 'changed'
}

/**
 * Pure batch replay decision:
 *   no existing batch            -> 'new'
 *   same batch id + same checksum -> 'replay' (safe replay)
 *   same batch id + diff checksum -> 'conflict' (truthful conflict)
 */
export function batchReplayDecision(
  existing: { file_sha256: string } | undefined,
  currentSha: string,
): 'new' | 'replay' | 'conflict' {
  if (!existing) return 'new'
  if (existing.file_sha256 === currentSha) return 'replay'
  return 'conflict'
}

type ContactOutcome = 'new' | 'replay' | 'changed' | 'error'

async function main(q: QueryExecutor) {
    const raw = await readFile(file!, 'utf8')
    const fileSha256 = createHash('sha256').update(raw).digest('hex')
  let batch: AppleContactExportBatch
  try {
    batch = parseAppleContactExportBatch(JSON.parse(raw))
  } catch (err) {
    console.error('Export parse/validation failed:', (err as Error).message)
    process.exit(1)
  }
  const inputCount = batch.contacts.length

  // ---- Batch receipt (identity: source + source_account + external_batch_id) --
  const batchRows = await q`
    insert into integration_intake_batch (
      source, source_account, external_batch_id, schema_version,
      exported_at, received_at, file_sha256, load_status,
      input_count, valid_count, new_profile_count, replay_count,
      changed_revision_count, error_count
    ) values (
      ${APPLE_CONTACTS_SOURCE_SYSTEM}, ${sourceAccount}, ${batch.exportId},
      ${batch.schemaVersion}, ${batch.exportedAt}, now(), ${fileSha256}, 'processing',
      0, 0, 0, 0, 0, 0
    )
    on conflict (source, source_account, external_batch_id) do nothing
    returning id
  `
  let batchId: string
  let batchCreated: boolean
  if (batchRows.length > 0) {
    batchId = String((batchRows[0] as { id: string }).id)
    batchCreated = true
  } else {
    const existing = await q`
      select id, file_sha256 from integration_intake_batch
      where source = ${APPLE_CONTACTS_SOURCE_SYSTEM}
        and source_account = ${sourceAccount}
        and external_batch_id = ${batch.exportId}
      limit 1
    `
    const row = existing[0] as { id: string; file_sha256: string } | undefined
    if (!row) {
      console.error('Batch receipt race; aborting (fail closed)')
      process.exit(1)
    }
    if (row.file_sha256 !== fileSha256) {
      await q`update integration_intake_batch set load_status='conflict', updated_at=now() where id = ${row.id}`
      console.error(
        `CONFLICT: batch ${batch.exportId} exists with a different checksum (safe replay only with the identical file).`,
      )
      process.exit(1)
    }
    batchId = row.id
    batchCreated = false
  }

  // ---- Stage each contact (idempotent, constraint-backed) ---------------------
  const counts = { new: 0, replay: 0, changed: 0, error: 0 }
  const manifest = {
    importId: batch.exportId,
    sourceSystem: APPLE_CONTACTS_SOURCE_SYSTEM,
    adapter: APPLE_CONTACTS_ADAPTER,
    adapterVersion: APPLE_CONTACTS_ADAPTER_VERSION,
    importedAt: new Date().toISOString(),
    sourceAccount,
  }

  for (const contact of batch.contacts) {
    const outcome = await stageContact(contact, q, manifest, batchId, batch.schemaVersion)
    counts[outcome]++
  }

  const validCount = counts.new + counts.replay + counts.changed
  await q`
    update integration_intake_batch set
      input_count = ${inputCount},
      valid_count = ${validCount},
      new_profile_count = ${counts.new},
      replay_count = ${counts.replay},
      changed_revision_count = ${counts.changed},
      error_count = ${counts.error},
      load_status = 'loaded',
      updated_at = now()
    where id = ${batchId}
  `

  // ---- Non-PII aggregate report -----------------------------------------------
  const profile = dataQualityProfile(batch.contacts)
  console.log(JSON.stringify({
    env,
    sourceAccount,
    source: APPLE_CONTACTS_SOURCE_SYSTEM,
    exportId: batch.exportId,
    fileSha256,
    batchCreated,
    batchId,
    totals: {
      input: inputCount,
      valid: validCount,
      new: counts.new,
      replay: counts.replay,
      changed: counts.changed,
      error: counts.error,
    },
    balanced: inputCount === validCount + counts.error && validCount === counts.new + counts.replay + counts.changed,
    dataQuality: profile,
  }, null, 2))
}


async function stageContact(
  contact: AppleContactExport,
  q: QueryExecutor,
  manifest: {
    importId: string
    sourceSystem: string
    adapter: string
    adapterVersion: string
    importedAt: string
    sourceAccount?: string
  },
  batchId: string,
  schemaVersion: number,
): Promise<ContactOutcome> {
  try {
    const lowered = lowerBatchItemToIntakeMessage(
      manifest,
      appleContactToBatchItem(contact, batchId, new Date().toISOString()),
    )
    const identity = {
      source: manifest.sourceSystem,
      sourceAccount: manifest.sourceAccount ?? '',
      externalEventId: contact.sourceId,
    }
    const inbox = await insertOrReadIntegrationInbox(
      {
        source: identity.source,
        sourceAccount: identity.sourceAccount,
        externalEventId: identity.externalEventId,
        eventType: lowered.eventType,
        occurredAt: lowered.occurredAt,
        observedAt: lowered.observedAt ?? new Date().toISOString(),
        direction: 'inbound',
        correlationId: lowered.correlationId ?? null,
        threadId: null,
        maxAttempts: 3,
        subject: ((lowered.content as { subject?: string } | undefined)?.subject ?? null) as string | null,
        summary: ((lowered.content as { summary?: string } | undefined)?.summary ?? null) as string | null,
        contentReference: lowered.provenance?.rawReference ?? null,
        provenanceReference: lowered.provenance?.rawReference ?? null,
        participantIdentities: lowered.participants.map((p) => ({
          kind: p.kind,
          value: p.value,
          displayName: p.displayName,
        })),
        contactCandidates: lowered.contactCandidates ?? [],
        attachmentMetadata: null,
      },
      q,
    )

    const profile = normalizeProfile(contact)
    const fingerprint = profileFingerprint(profile)
    const prior = await q`
      select id, revision, payload_fingerprint from integration_staged_contact_profile
      where source = ${identity.source}
        and source_account = ${identity.sourceAccount}
        and source_contact_id = ${identity.externalEventId}
      order by revision desc
      limit 1
    `
    const priorRow = prior[0] as { id: string; revision: number; payload_fingerprint: string } | undefined

    if (decideStagingOutcome(priorRow, fingerprint) === 'replay') {
      return 'replay' // exact replay: no new revision
    }
    const nextRevision = priorRow ? priorRow.revision + 1 : 1
    const supersedesId = priorRow ? priorRow.id : null
    const inserted = await q`
      insert into integration_staged_contact_profile (
        integration_inbox_id, integration_intake_batch_id,
        source, source_account, source_contact_id, revision, schema_version,
        payload_fingerprint, profile, supersedes_profile_id
      ) values (
        ${inbox.record.id}, ${batchId}, ${identity.source}, ${identity.sourceAccount},
        ${identity.externalEventId}, ${nextRevision}, ${schemaVersion},
        ${fingerprint}, ${JSON.stringify(profile)}::jsonb, ${supersedesId}
      )
      on conflict (source, source_account, source_contact_id, payload_fingerprint) do nothing
      returning id
    `
    if (inserted.length === 0) return 'replay' // concurrent exact replay
    return priorRow ? 'changed' : 'new'
  } catch (err) {
    console.error(`Staging failed for contact sourceId: ${contact.sourceId}`, (err as Error).message)
    return 'error'
  }
}

function dataQualityProfile(contacts: AppleContactExport[]) {
  const personal = (c: AppleContactExport) =>
    [c.namePrefix, c.givenName, c.middleName, c.familyName, c.nameSuffix]
      .map((p) => p.trim()).filter(Boolean).join(' ')
  return {
    total: contacts.length,
    withEmail: contacts.filter((c) => c.emails.length > 0).length,
    withPhone: contacts.filter((c) => c.phones.length > 0).length,
    withPostalAddress: contacts.filter((c) => c.postalAddresses.length > 0).length,
    withMultipleEmails: contacts.filter((c) => c.emails.length > 1).length,
    withMultiplePhones: contacts.filter((c) => c.phones.length > 1).length,
    withOrganization: contacts.filter((c) => c.organization.trim() !== '').length,
    withNeitherEmailNorPhone: contacts.filter((c) => c.emails.length === 0 && c.phones.length === 0).length,
    organizationOrSourceIdDisplayFallback: contacts.filter(
      (c) => personal(c) === '' && (c.organization.trim() !== '' || c.sourceId !== ''),
    ).length,
  }
}
