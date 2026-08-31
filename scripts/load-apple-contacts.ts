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
  let errorCount = 0
  try {
    errorCount = (await main(q)).errorCount
  } finally {
    await pool.end()
  }
  if (errorCount > 0) {
    console.error(
      `[contacts-sync] ${errorCount} contact(s) failed; batch marked failed; exiting non-zero after durable accounting`,
    )
    process.exit(1)
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

/**
 * Pure classification for the replay fast path.
 *   prior latest revision exists + same fingerprint + durable inbox receipt exists
 *     -> 'replay' (exact replay: NO new staged revision, NO redundant DB writes)
 *   otherwise -> 'write' (new / changed / replay-with-inbox-gap still needs the
 *     heavier inbox + staging write path)
 */
export function classifyReplayFastPath(
  prior: { payload_fingerprint: string } | undefined,
  fingerprint: string,
  inboxExists: boolean,
): 'replay' | 'write' {
  if (prior && prior.payload_fingerprint === fingerprint && inboxExists) return 'replay'
  return 'write'
}

/** Truthful batch load status: any per-contact error marks the batch NOT fully loaded. */
export function decideBatchLoadStatus(errorCount: number): 'loaded' | 'failed' {
  return errorCount > 0 ? 'failed' : 'loaded'
}

/** Exit code after durable batch accounting: non-zero when any contact failed. */
export function decideLoaderExit(errorCount: number): 0 | 1 {
  return errorCount > 0 ? 1 : 0
}

/** Totals + balance for the batch (mirrors the integration_intake_batch CHECK). */
export function batchTotals(
  counts: { new: number; replay: number; changed: number; error: number },
  inputCount: number,
): { valid: number; balanced: boolean } {
  const valid = counts.new + counts.replay + counts.changed
  return {
    valid,
    balanced: inputCount === valid + counts.error && valid === counts.new + counts.replay + counts.changed,
  }
}

/** Next immutable revision + supersedes link for a changed/new write. */
export function stageWriteDecision(
  prior: { id: string; revision: number } | undefined,
): { nextRevision: number; supersedesId: string | null } {
  return {
    nextRevision: prior ? prior.revision + 1 : 1,
    supersedesId: prior ? prior.id : null,
  }
}

/** Aggregate progress line — never any PII (no names/emails/phones/sourceIds). */
export function formatProgress(processed: number, total: number, elapsedMs: number): string {
  const secs = Math.round(elapsedMs / 1000)
  return `[contacts-sync] ODS ${processed}/${total} processed (${secs}s)`
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
  const startMs = Date.now()

  // Phase A — normalize + fingerprint every contact locally (no DB round trips).
  const prepared = batch.contacts.map((contact) => {
    const profile = normalizeProfile(contact)
    return {
      contact,
      sourceId: contact.sourceId,
      profile,
      fingerprint: profileFingerprint(profile),
    }
  })

  // Phase B — bulk-read the existing latest staged profile + durable inbox
  // receipts for this source_account in bounded chunks, so exact replays can be
  // classified in memory without per-contact SELECTs.
  const stagedBySourceId = new Map<
    string,
    { id: string; revision: number; payload_fingerprint: string }
  >()
  const inboxExisting = new Set<string>()
  const BULK_CHUNK = 400
  for (let i = 0; i < prepared.length; i += BULK_CHUNK) {
    const ids = prepared.slice(i, i + BULK_CHUNK).map((p) => p.sourceId)
    const stagedRows = (await q`
      select distinct on (source_contact_id) source_contact_id, id, revision, payload_fingerprint
      from integration_staged_contact_profile
      where source = ${APPLE_CONTACTS_SOURCE_SYSTEM}
        and source_account = ${sourceAccount}
        and source_contact_id = any(${ids}::text[])
      order by source_contact_id, revision desc
    `) as Array<{
      source_contact_id: string
      id: string
      revision: number
      payload_fingerprint: string
    }>
    for (const r of stagedRows) {
      stagedBySourceId.set(r.source_contact_id, {
        id: r.id,
        revision: Number(r.revision),
        payload_fingerprint: r.payload_fingerprint,
      })
    }
    const inboxRows = (await q`
      select external_event_id from integration_inbox
      where source = ${APPLE_CONTACTS_SOURCE_SYSTEM}
        and source_account = ${sourceAccount}
        and external_event_id = any(${ids}::text[])
    `) as Array<{ external_event_id: string }>
    for (const r of inboxRows) inboxExisting.add(r.external_event_id)
  }

  // Phase C — classify: pure exact replays skip all per-contact DB work; every
  // new / changed / replay-with-inbox-gap contact goes through the durable write
  // path (inbox receipt + staged revision) exactly as before.
  let processed = 0
  for (const p of prepared) {
    const prior = stagedBySourceId.get(p.sourceId)
    if (classifyReplayFastPath(prior, p.fingerprint, inboxExisting.has(p.sourceId)) === 'replay') {
      counts.replay++ // fast path: no new staged revision, durable inbox already present
    } else {
      const outcome = await stageContact(p.contact, q, manifest, batchId, batch.schemaVersion)
      counts[outcome]++
    }
    processed++
    if (processed % 500 === 0 || processed === prepared.length) {
      console.log(formatProgress(processed, prepared.length, Date.now() - startMs))
    }
  }

  // ---- Snapshot membership (durably recorded BEFORE the batch is marked
  // loaded). Every contact present in THIS successful export is a member of the
  // current snapshot — INCLUDING exact replays, whose latest staged profile
  // revision may belong to an older batch. Membership is never derived from old
  // staged rows; it records the actual current export. Set-based + idempotent.
  const memberIds = prepared.map((p) => p.sourceId)
  await q`
    insert into integration_source_snapshot_member (
      integration_intake_batch_id, source, source_account, source_identity_key
    )
    select ${batchId}, ${APPLE_CONTACTS_SOURCE_SYSTEM}, ${sourceAccount}, unnest(${memberIds}::text[])
    on conflict (integration_intake_batch_id, source, source_account, source_identity_key) do nothing
  `

  const validCount = counts.new + counts.replay + counts.changed
  const loadStatus = decideBatchLoadStatus(counts.error)
  await q`
    update integration_intake_batch set
      input_count = ${inputCount},
      valid_count = ${validCount},
      new_profile_count = ${counts.new},
      replay_count = ${counts.replay},
      changed_revision_count = ${counts.changed},
      error_count = ${counts.error},
      load_status = ${loadStatus},
      updated_at = now()
    where id = ${batchId}
  `

  // ---- Non-PII aggregate report -----------------------------------------------
  const profile = dataQualityProfile(batch.contacts)
  const totals = batchTotals(counts, inputCount)
  console.log(JSON.stringify({
    env,
    sourceAccount,
    source: APPLE_CONTACTS_SOURCE_SYSTEM,
    exportId: batch.exportId,
    fileSha256,
    batchCreated,
    batchId,
    loadStatus,
    totals: {
      input: inputCount,
      valid: totals.valid,
      new: counts.new,
      replay: counts.replay,
      changed: counts.changed,
      error: counts.error,
    },
    balanced: totals.balanced,
    dataQuality: profile,
  }, null, 2))

  return { errorCount: counts.error }
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
