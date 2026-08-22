import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

// ---------------------------------------------------------------------------
// INTAKE-01 — Canonical Intake Message Contract.
//
// One normalized intake message; two acquisition lanes (batch + realtime); one
// transformation stack. Scoped per the runtime test policy: this file only —
// no full regression, no persistence harness. It proves the acceptance
// criteria against pure in-memory seams:
//   1. the contract is explicit and source-neutral (both lanes emit the SAME
//      canonical envelope surface);
//   2. duplicate/replay identity is defined — (source.system, source.itemId),
//      lane/account/batch-agnostic, and the durable inbox key is derived from
//      it;
//   3. ownership of provenance/raw source is clear — the lowering adapter
//      owns raw translation; canonical CRM stores references only;
//   4. batch and realtime can both emit the envelope and both lower into the
//      SAME durable inbox insert (equivalent facts project identically);
//   5. downstream code needs no source-specific parsing rules — the inbox
//      insert is the one neutral projection, sourcePayload never leaks;
//   6. no new canonical CRM state model is created — the projection targets
//      the EXISTING integration inbox contract and lib/intake contains no
//      SQL/table.
// ---------------------------------------------------------------------------

import { contactsFixture, lowerRawObservation } from '../../lib/mac-observer'
import type {
  ExternalActivityEvent,
  RawObservation,
} from '../../lib/mac-observer/contracts'
import {
  lowerExternalActivityEventToIntakeMessage,
} from '../../lib/intake/realtime'
import { lowerBatchItemToIntakeMessage } from '../../lib/intake/batch'
import type {
  IntakeBatchItemInput,
  IntakeBatchManifest,
} from '../../lib/intake/batch'
import { toInboxInsert } from '../../lib/intake/inbox'
import {
  INTAKE_SOURCE_PAYLOAD_MAX_BYTES,
  assertValidIntakeMessage,
  intakeDedupeKey,
  intakeSourceIdentity,
  validateIntakeMessage,
} from '../../lib/intake/contracts'
import type { CanonicalIntakeMessage } from '../../lib/intake/contracts'
import {
  DEFAULT_INTEGRATION_INBOX_CONFIGURATION,
  processExternalActivityEvent,
} from '../../lib/integration-inbox/processor'
import type { MacIntakeRepositories } from '../../lib/integration-inbox/processor'
import type {
  InsertIntegrationInboxInput,
  IntegrationInboxDurability,
  IntegrationInboxRecord,
} from '../../lib/integration-inbox/contracts'

const FIXED_NOW = '2026-08-22T00:00:00.000Z'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function realtimeContactsEvent(): ExternalActivityEvent {
  return lowerRawObservation({
    source: 'contacts',
    sourceAccount: 'iCloud:acct',
    rawEventId: 'contact-1',
    observedAt: '2026-08-22T10:05:00.000Z',
    payload: contactsFixture('1').payload as RawObservation['payload'],
  })
}

const batchManifest: IntakeBatchManifest = {
  importId: 'import-2026-08-22-contacts',
  sourceSystem: 'import:csv:contacts-v1',
  adapter: 'csv-import.v1',
  adapterVersion: '1.0.0',
  importedAt: '2026-08-22T11:00:00.000Z',
}

function batchItem(): IntakeBatchItemInput {
  return {
    itemId: 'row-42',
    eventType: 'contact.imported',
    occurredAt: '2026-08-21T09:00:00.000Z',
    participants: [
      {
        kind: 'email',
        value: 'buyer1@example.com',
        displayName: 'Buyer 1',
        role: 'subject',
      },
    ],
    contactCandidates: [
      { kind: 'email', value: 'buyer1@example.com' },
      { kind: 'phone', value: '+15550100001' },
    ],
    content: { subject: 'Imported contact', summary: 'CSV row 42' },
    thread: { conversationId: 'import-conv-42' },
    attachments: [
      { referenceId: 'att-42', filename: 'plans.pdf', mimeType: 'application/pdf', sizeBytes: 2048 },
    ],
    correlationId: 'corr-import-42',
    causationId: 'command-7',
    rawReference: 'imports/2026-08-22/contacts.csv#row-42',
    sourcePayload: { sourceRow: 42, note: 'bounded import extras' },
  }
}

/** The canonical envelope key surface (all optional keys included). */
const CONTRACT_KEYS = [
  'schemaVersion', 'acquisitionLane', 'source', 'eventType', 'occurredAt',
  'observedAt', 'direction', 'participants', 'contactCandidates', 'thread',
  'content', 'attachments', 'context', 'correlationId', 'causationId',
  'provenance', 'sourcePayload',
].sort()

// ---------------------------------------------------------------------------
// 1. Explicit, source-neutral contract (criteria 1 + 4)
// ---------------------------------------------------------------------------

test('both lanes emit the SAME canonical envelope surface — explicit and source-neutral', () => {
  const realtime = lowerExternalActivityEventToIntakeMessage(realtimeContactsEvent())
  const batch = lowerBatchItemToIntakeMessage(batchManifest, batchItem())

  // Every key either lane emits is a contract key, and the union of the two
  // lanes' emitted keys IS the contract surface — the envelope is one shape,
  // not two lane-specific shapes.
  for (const key of Object.keys(realtime)) {
    assert.ok(CONTRACT_KEYS.includes(key), `realtime key '${key}' is contract surface`)
  }
  for (const key of Object.keys(batch)) {
    assert.ok(CONTRACT_KEYS.includes(key), `batch key '${key}' is contract surface`)
  }
  assert.deepEqual(
    [...new Set([...Object.keys(realtime), ...Object.keys(batch)])].sort(),
    CONTRACT_KEYS,
  )

  assert.equal(realtime.schemaVersion, 1)
  assert.equal(batch.schemaVersion, 1)
  assert.equal(realtime.acquisitionLane, 'realtime')
  assert.equal(batch.acquisitionLane, 'batch')

  // The contract validation is lane-agnostic: both lanes pass the same rules.
  assert.deepEqual(validateIntakeMessage(realtime), [])
  assert.deepEqual(validateIntakeMessage(batch), [])
  assertValidIntakeMessage(realtime)
  assertValidIntakeMessage(batch)

  // Envelope fields the architect brief names, present on both lanes.
  for (const message of [realtime, batch]) {
    assert.ok(message.source.system.length > 0, 'stable source system')
    assert.ok(message.source.itemId.length > 0, 'source item/event id')
    assert.ok(!Number.isNaN(new Date(message.occurredAt).getTime()), 'occurred_at')
    assert.ok(message.participants.length > 0, 'participant identities')
    assert.ok(message.provenance.adapter.length > 0, 'provenance adapter')
    assert.ok(message.provenance.adapterVersion.length > 0, 'provenance version')
  }
})

// ---------------------------------------------------------------------------
// 2. Duplicate / replay identity (criterion 2)
// ---------------------------------------------------------------------------

test('duplicate/replay identity is (source.system, source.itemId) — lane, account and batch agnostic', () => {
  const realtime = lowerExternalActivityEventToIntakeMessage(realtimeContactsEvent())

  // The SAME source fact arriving through the batch lane (an import of the
  // observer's data) carries the SAME canonical identity.
  const batchSameFact = lowerBatchItemToIntakeMessage(
    { ...batchManifest, sourceSystem: 'contacts' },
    { ...batchItem(), itemId: 'contact-1', occurredAt: realtime.occurredAt },
  )
  assert.equal(intakeDedupeKey(realtime), intakeDedupeKey(batchSameFact))

  // A re-run of the same import (new batch/import id) keeps the same identity:
  // the batch id is provenance, never identity.
  const rerun = lowerBatchItemToIntakeMessage(
    { ...batchManifest, importId: 'import-2026-08-23-contacts' },
    batchItem(),
  )
  assert.equal(intakeDedupeKey(rerun), intakeDedupeKey(lowerBatchItemToIntakeMessage(batchManifest, batchItem())))

  // A different source item id is a different identity.
  const otherRow = lowerBatchItemToIntakeMessage(
    batchManifest,
    { ...batchItem(), itemId: 'row-43' },
  )
  assert.notEqual(
    intakeDedupeKey(otherRow),
    intakeDedupeKey(lowerBatchItemToIntakeMessage(batchManifest, batchItem())),
  )
})

test('durable inbox dedupe key is derived from the canonical identity', () => {
  const realtime = lowerExternalActivityEventToIntakeMessage(realtimeContactsEvent())
  assert.deepEqual(intakeSourceIdentity(realtime), {
    source: 'contacts',
    sourceAccount: 'iCloud:acct',
    externalEventId: 'contact-1',
  })

  const batch = lowerBatchItemToIntakeMessage(batchManifest, batchItem())
  // Batch leaves the account empty so the durable key equals the canonical
  // (system, itemId): re-imports replay, never duplicate.
  assert.deepEqual(intakeSourceIdentity(batch), {
    source: 'import:csv:contacts-v1',
    sourceAccount: '',
    externalEventId: 'row-42',
  })

  const realtimeInsert = toInboxInsert(realtime, 3)
  const batchInsert = toInboxInsert(batch, 3)
  assert.equal(realtimeInsert.source, 'contacts')
  assert.equal(realtimeInsert.sourceAccount, 'iCloud:acct')
  assert.equal(realtimeInsert.externalEventId, 'contact-1')
  assert.equal(batchInsert.source, 'import:csv:contacts-v1')
  assert.equal(batchInsert.sourceAccount, '')
  assert.equal(batchInsert.externalEventId, 'row-42')
})

// ---------------------------------------------------------------------------
// 3. Provenance / raw source ownership (criterion 3)
// ---------------------------------------------------------------------------

test('provenance ownership is explicit: adapter + version name the owner; the inbox stores references only', () => {
  const realtime = lowerExternalActivityEventToIntakeMessage(realtimeContactsEvent())
  assert.equal(realtime.provenance.adapter, 'contacts.v1')
  assert.equal(realtime.provenance.adapterVersion, 'contacts.v1')
  assert.equal(realtime.provenance.rawReference, 'contacts-raw-1.json')

  const batch = lowerBatchItemToIntakeMessage(batchManifest, batchItem())
  assert.equal(batch.provenance.adapter, 'csv-import.v1')
  assert.equal(batch.provenance.adapterVersion, '1.0.0')
  assert.equal(batch.provenance.rawReference, 'imports/2026-08-22/contacts.csv#row-42')

  const batchInsert = toInboxInsert(batch, 3)
  assert.equal(batchInsert.provenanceReference, 'imports/2026-08-22/contacts.csv#row-42')
  // The raw payload is never stored: sourcePayload exists on the envelope but
  // the durable inbox insert has no payload surface at all.
  assert.ok(!('sourcePayload' in batchInsert), 'sourcePayload never reaches the inbox')
  const serialized = JSON.stringify(batchInsert)
  assert.ok(!serialized.includes('bounded import extras'), 'raw/bounded payload bytes never persist')
})

// ---------------------------------------------------------------------------
// 4. Batch and realtime both emit + ONE downstream projection (criteria 4 + 5)
// ---------------------------------------------------------------------------

test('downstream needs NO source-specific parsing: equivalent facts project identically for both lanes', () => {
  // The SAME neutral fact — identical participants, candidates, thread,
  // content, attachments, occurredAt — arriving via realtime vs batch.
  const shared = {
    participants: [
      { kind: 'email', value: 'buyer1@example.com', displayName: 'Buyer 1', role: 'sender' as const },
      { kind: 'email', value: 'agent@culebraluxe.example', role: 'recipient' as const },
    ],
    contactCandidates: [
      { kind: 'email', value: 'buyer1@example.com' },
      { kind: 'phone', value: '+15550100001' },
    ],
    thread: { id: 'thread-1', conversationId: 'conv-1', inReplyTo: 'prev-1' },
    content: { subject: 'Property inquiry', summary: 'Interested in Casa Luar.', contentReference: 'imap-uid-1' },
    attachments: [
      { referenceId: 'att-1', filename: 'plans.pdf', mimeType: 'application/pdf', sizeBytes: 2048 },
    ],
    occurredAt: '2026-08-22T10:14:00.000Z',
    correlationId: 'corr-1',
  }

  const realtimeEvent: ExternalActivityEvent = {
    schemaVersion: 1,
    source: 'mail',
    sourceAccount: 'IMAP:acct',
    externalEventId: 'msg-1',
    eventType: 'mail.message_received',
    occurredAt: shared.occurredAt,
    observedAt: '2026-08-22T10:15:00.000Z',
    direction: 'inbound',
    participants: shared.participants,
    contactCandidates: shared.contactCandidates,
    thread: shared.thread,
    content: shared.content,
    attachments: shared.attachments,
    correlationId: shared.correlationId,
    provenance: { adapter: 'mail.v1', adapterVersion: 'mail.v1', rawReference: 'mail-raw-1.eml' },
  }
  const realtime = lowerExternalActivityEventToIntakeMessage(realtimeEvent)
  const batch = lowerBatchItemToIntakeMessage(batchManifest, {
    itemId: 'row-42',
    eventType: 'mail.message_received',
    occurredAt: shared.occurredAt,
    direction: 'inbound',
    participants: shared.participants,
    contactCandidates: shared.contactCandidates,
    thread: shared.thread,
    content: shared.content,
    attachments: shared.attachments,
    correlationId: shared.correlationId,
    rawReference: 'imports/2026-08-22/mail.csv#row-42',
  })

  const realtimeInsert = toInboxInsert(realtime, 3)
  const batchInsert = toInboxInsert(batch, 3)

  // Every neutral downstream field is byte-identical between lanes — the SAME
  // downstream code (inbox, identity resolution, interaction.record) consumes
  // both with zero source-specific parsing.
  assert.deepEqual(realtimeInsert.participantIdentities, batchInsert.participantIdentities)
  assert.deepEqual(realtimeInsert.contactCandidates, batchInsert.contactCandidates)
  assert.deepEqual(realtimeInsert.threadId, batchInsert.threadId)
  assert.deepEqual(realtimeInsert.subject, batchInsert.subject)
  assert.deepEqual(realtimeInsert.summary, batchInsert.summary)
  assert.deepEqual(realtimeInsert.contentReference, batchInsert.contentReference)
  assert.deepEqual(realtimeInsert.attachmentMetadata, batchInsert.attachmentMetadata)
  assert.deepEqual(realtimeInsert.occurredAt, batchInsert.occurredAt)
  assert.deepEqual(realtimeInsert.direction, batchInsert.direction)
  assert.deepEqual(realtimeInsert.eventType, batchInsert.eventType)

  // Only the source-identity fields differ (the edge), and the correlation
  // id is preserved from each envelope.
  assert.notEqual(realtimeInsert.source, batchInsert.source)
  assert.equal(realtimeInsert.correlationId, 'corr-1')
  assert.equal(batchInsert.correlationId, 'corr-1')
})

test('toInboxInsert returns exactly the EXISTING integration inbox insert surface (no new state model)', () => {
  const batch = lowerBatchItemToIntakeMessage(batchManifest, batchItem())
  const insert = toInboxInsert(batch, 3)

  const inboxKeys = [
    'source', 'sourceAccount', 'externalEventId', 'eventType', 'occurredAt',
    'observedAt', 'direction', 'correlationId', 'threadId', 'subject', 'summary',
    'contentReference', 'provenanceReference', 'participantIdentities',
    'contactCandidates', 'attachmentMetadata', 'maxAttempts',
  ].sort()
  assert.deepEqual(Object.keys(insert).sort(), inboxKeys)
  assert.equal(insert.maxAttempts, 3)
  assert.equal(insert.observedAt, batchManifest.importedAt)
})

// ---------------------------------------------------------------------------
// 5. Validation: contract invariants + bounded payload (criterion 1)
// ---------------------------------------------------------------------------

test('validation enforces contract invariants; sourcePayload is bounded to 32 KB', () => {
  const valid = lowerBatchItemToIntakeMessage(batchManifest, batchItem())
  assert.deepEqual(validateIntakeMessage(valid), [])

  const cases: Array<[CanonicalIntakeMessage, RegExp]> = [
    [{ ...valid, schemaVersion: 2 as never }, /schemaVersion/],
    [{ ...valid, acquisitionLane: 'carrier' as never }, /acquisitionLane/],
    [{ ...valid, source: { ...valid.source, system: '' } }, /source\.system/],
    [{ ...valid, source: { ...valid.source, itemId: '  ' } }, /source\.itemId/],
    [{ ...valid, eventType: '' }, /eventType/],
    [{ ...valid, occurredAt: 'not-a-date' }, /occurredAt/],
    [{ ...valid, provenance: { ...valid.provenance, adapter: '' } }, /provenance\.adapter/],
    [{ ...valid, provenance: { ...valid.provenance, adapterVersion: '' } }, /provenance\.adapterVersion/],
    [
      { ...valid, sourcePayload: { big: 'x'.repeat(INTAKE_SOURCE_PAYLOAD_MAX_BYTES) } },
      /exceeds the \d+-byte bound/,
    ],
  ]
  for (const [message, pattern] of cases) {
    const problems = validateIntakeMessage(message)
    assert.ok(problems.length > 0, `expected problems for ${pattern}`)
    assert.ok(problems.some((p) => pattern.test(p)), `${pattern} in ${problems.join('; ')}`)
    assert.throws(() => assertValidIntakeMessage(message), pattern)
  }

  // A payload at the bound passes; just over fails.
  const atBound: CanonicalIntakeMessage = {
    ...valid,
    sourcePayload: { data: 'x'.repeat(INTAKE_SOURCE_PAYLOAD_MAX_BYTES - 32) },
  }
  assert.deepEqual(validateIntakeMessage(atBound), [])
})

// ---------------------------------------------------------------------------
// 6. No new canonical CRM state model (criterion 6)
// ---------------------------------------------------------------------------

test('lib/intake defines a message contract only — no SQL, no table, no migration', () => {
  for (const file of ['contracts.ts', 'realtime.ts', 'batch.ts', 'inbox.ts', 'index.ts']) {
    const source = readFileSync(join(__dirname, '../../lib/intake', file), 'utf8')
    assert.ok(!/create table|create migration|db\/migrations/i.test(source), file)
  }
})

// ---------------------------------------------------------------------------
// 7. Processor convergence: the realtime lane emits the canonical message
//    (criterion 4 — the durable receipt derives from the canonical envelope)
// ---------------------------------------------------------------------------

type Row = Record<string, any>

/** Minimal in-memory intake repositories (identity resolution) for the
 *  contacts convergence path. */
class MinimalDb {
  identities: Row[] = []

  seedPerson(personId: string, identities: Array<{ kind: string; value: string }>) {
    for (const i of identities) {
      this.identities.push({
        identity_id: `${personId}:${i.kind}:${i.value}`,
        person_id: personId,
        kind: i.kind,
        normalized_value: i.value,
        archived: false,
      })
    }
  }

  async findInteractionBySourceIdentity() {
    return null
  }

  async personExists() {
    return true
  }

  async findIdentityMatch(hint: { kind: string; normalizedValue: string }) {
    const row = this.identities.find(
      (r) => r.kind === hint.kind && r.normalized_value === hint.normalizedValue && !r.archived,
    )
    return row
      ? { identityId: row.identity_id, personId: row.person_id, kind: row.kind, normalizedValue: row.normalized_value }
      : null
  }

  async findIdentityOwnership(hint: { kind: string; normalizedValue: string }) {
    const row = this.identities.find(
      (r) => r.kind === hint.kind && r.normalized_value === hint.normalizedValue,
    )
    return row
      ? { ...(await this.findIdentityMatch(hint))!, archived: row.archived === true }
      : null
  }

  async createPersonWithIdentities() {
    throw new Error('person auto-creation is never enabled on this path')
  }

  async findPropertyById() {
    return null
  }

  async findPropertyBySlug() {
    return null
  }

  async findDealById() {
    return null
  }

  repositories(): MacIntakeRepositories {
    return {
      findInteractionBySourceIdentity: (s, e) => this.findInteractionBySourceIdentity(s, e),
      personExists: (id) => this.personExists(id),
      findIdentityMatch: (hint) => this.findIdentityMatch(hint),
      findIdentityOwnership: (hint) => this.findIdentityOwnership(hint),
      createPersonWithIdentities: (input) => this.createPersonWithIdentities(input),
      findPropertyById: () => this.findPropertyById(),
      findPropertyBySlug: () => this.findPropertyBySlug(),
      findDealById: () => this.findDealById(),
    }
  }
}

/** Durable-inbox double that RECORDS the insert the processor attempts and
 *  returns the canonical record shape for the claim/transition lifecycle. */
function recordingDurability(
  recorded: InsertIntegrationInboxInput[],
): IntegrationInboxDurability {
  let seq = 0
  const record = (overrides: Partial<IntegrationInboxRecord> = {}): IntegrationInboxRecord => ({
    id: `inbox-${++seq}`,
    source: '',
    sourceAccount: '',
    externalEventId: '',
    eventType: '',
    occurredAt: FIXED_NOW,
    observedAt: FIXED_NOW,
    direction: null,
    correlationId: null,
    threadId: null,
    subject: null,
    summary: null,
    contentReference: null,
    provenanceReference: null,
    participantIdentities: [],
    contactCandidates: null,
    attachmentMetadata: null,
    status: 'received',
    attemptCount: 0,
    maxAttempts: 3,
    lastError: null,
    processingStartedAt: null,
    processingCompletedAt: null,
    resolvedPersonId: null,
    interactionId: null,
    createdAt: FIXED_NOW,
    updatedAt: FIXED_NOW,
    ...overrides,
  })
  let current = record()

  return {
    async insertOrReadReceipt(input) {
      recorded.push(input)
      current = record({
        source: input.source,
        sourceAccount: input.sourceAccount,
        externalEventId: input.externalEventId,
        eventType: input.eventType,
        occurredAt: input.occurredAt,
        observedAt: input.observedAt,
        direction: input.direction,
        correlationId: input.correlationId,
        threadId: input.threadId,
        subject: input.subject,
        summary: input.summary,
        contentReference: input.contentReference,
        provenanceReference: input.provenanceReference,
        participantIdentities: input.participantIdentities,
        contactCandidates: input.contactCandidates,
        attachmentMetadata: input.attachmentMetadata,
        maxAttempts: input.maxAttempts,
      })
      return { record: current, created: true }
    },
    async claimReceipt(receiptId) {
      if (current.id !== receiptId) return null
      current = { ...current, status: 'processing', processingStartedAt: FIXED_NOW }
      return current
    },
    async transitionReceipt(input) {
      if (
        current.id !== input.receiptId ||
        current.status !== input.from ||
        current.processingStartedAt !== input.claimToken
      ) {
        return false
      }
      current = {
        ...current,
        status: input.to,
        interactionId: input.interactionId ?? null,
        resolvedPersonId: input.resolvedPersonId ?? null,
        processingStartedAt: null,
        processingCompletedAt: FIXED_NOW,
      }
      return true
    },
    async failReceipt(input) {
      current = { ...current, status: 'received', attemptCount: input.attempts, lastError: input.error }
      return current
    },
    async listPending() {
      return []
    },
    async listPoisoned() {
      return []
    },
    async persistInteraction(input) {
      return { interactionId: `interaction-${++seq}`, created: true }
    },
  }
}

test('processor emits the canonical message: the durable insert equals the canonical projection', async () => {
  const db = new MinimalDb()
  db.seedPerson('person-1', [{ kind: 'email', value: 'buyer1@example.com' }])
  const event = realtimeContactsEvent()

  const recorded: InsertIntegrationInboxInput[] = []
  const durability = recordingDurability(recorded)

  const result = await processExternalActivityEvent({
    event,
    configuration: {
      ...DEFAULT_INTEGRATION_INBOX_CONFIGURATION,
      capabilities: {
        contacts: {
          status: 'available',
          reason: 'fake-for-test',
          requiredAccess: [],
          supportedAppleFrameworks: [],
        },
      },
    },
    repositories: db.repositories(),
    durability,
    channels: {
      calendar: { ownedCalendarEmails: [] },
      mail: { internalMailboxes: [] },
      messages: { ownedLines: [] },
      whatsapp: { ownedLines: [] },
    },
    now: () => FIXED_NOW,
  })

  assert.equal(result.outcome, 'completed')
  assert.equal(recorded.length, 1)

  // The durable receipt the processor writes is EXACTLY the canonical
  // projection — batch and realtime converge on the same transformation.
  assert.deepEqual(
    recorded[0],
    toInboxInsert(lowerExternalActivityEventToIntakeMessage(event), 3),
  )
})
