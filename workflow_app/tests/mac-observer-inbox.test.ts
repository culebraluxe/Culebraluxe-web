import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

// ---------------------------------------------------------------------------
// CRM-23 — macOS External Activity Observer + Durable Integration Inbox.
//
// Scoped per the runtime test policy: this file only — no full regression, no
// persistence harness. It proves, against an in-memory FakeDb + fake
// observers, the ten acceptance criteria:
//   1. a common observer contract represents contacts/calendar/mail/messages/
//      whatsapp events without source-specific fields leaking into CRM;
//   2. every observed event carries stable identity, occurredAt, source,
//      participants, thread, provenance and correlation metadata;
//   3. the Integration Inbox dedupes the same external event and replays
//      safely;
//   4. the observer is acquisition-only — no deal/task/workflow/alert
//      decisions;
//   5. identity/contact resolution maps external identities onto canonical
//      persons before any mutation;
//   6. normalized events reuse the existing CRM intake stubs (no parallel
//      canonical tables);
//   7. CRM changes happen through the canonical Business Command layer
//      (interaction.record);
//   8. unsupported/inaccessible source APIs are represented honestly;
//   9. poison/repeated-failure events are isolated/escalated without blocking
//      all intake;
//  10. raw sensitive payload retention is minimized and policy-driven.
// ---------------------------------------------------------------------------

import {
  createFakeMacObserver,
  contactsFixture,
  calendarFixture,
  mailFixture,
  lowerRawObservation,
  MacIntegrationObserver,
  contactsCapability,
  calendarCapability,
  mailCapability,
  messagesCapability,
  whatsappCapability,
} from '../../lib/mac-observer'
import type {
  ExternalActivityEvent,
  RawObservation,
} from '../../lib/mac-observer/contracts'
import {
  processExternalActivityEvent,
  syncMacObservations,
  DEFAULT_INTEGRATION_INBOX_CONFIGURATION,
} from '../../lib/integration-inbox/processor'
import type {
  MacChannelConfigurations,
  MacIntakeRepositories,
} from '../../lib/integration-inbox/processor'
import type { IntegrationInboxDurability } from '../../lib/integration-inbox/contracts'
import { createMacCapabilities } from '../../lib/integration-inbox/wiring'
import { createIntegrationInboxDurability } from '../../db/integration-inbox'
import type { QueryExecutor } from '../../db/query-executor'
import type {
  IdentityMatch,
  NormalizedIdentityHint,
} from '../../lib/crm-intake-types'
import type { Interaction } from '../../lib/crm-types'
import {
  INTERACTION_RECORD,
  RecordInteractionCommand,
} from '../../lib/commands/interaction/record-interaction'
import { InMemoryCommandRegistry } from '../../lib/commands/registry'
import { InMemoryDomainEventCollector } from '../../lib/commands/domain-events'
import type { CommandEnvelope } from '../../lib/workflow/contracts'

const FIXED_NOW = '2026-08-22T00:00:00.000Z'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function lowered(raw: RawObservation): ExternalActivityEvent {
  return lowerRawObservation(raw)
}

function calendarEvent(): ExternalActivityEvent {
  return lowered({
    source: 'calendar',
    sourceAccount: 'iCloud:acct',
    rawEventId: 'EKEvent-1',
    observedAt: '2026-08-22T10:10:00.000Z',
    payload: calendarFixture('1').payload as RawObservation['payload'],
  })
}

function mailEvent(): ExternalActivityEvent {
  return lowered({
    source: 'mail',
    sourceAccount: 'IMAP:acct',
    rawEventId: 'msg-1',
    observedAt: '2026-08-22T10:15:00.000Z',
    payload: mailFixture('1').payload as RawObservation['payload'],
  })
}

function contactsEvent(idSuffix = '1'): ExternalActivityEvent {
  return lowered({
    source: 'contacts',
    sourceAccount: 'iCloud:acct',
    rawEventId: `contact-${idSuffix}`,
    observedAt: '2026-08-22T10:05:00.000Z',
    payload: contactsFixture(idSuffix).payload as RawObservation['payload'],
  })
}

const channelConfigurations: MacChannelConfigurations = {
  calendar: {
    ownedCalendarEmails: [{ email: 'agent@culebraluxe.example' }],
  },
  mail: {
    internalMailboxes: [{ email: 'agent@culebraluxe.example' }],
  },
  messages: {
    ownedLines: [{ phone: '+15550100000' }],
  },
  whatsapp: {
    ownedLines: [],
  },
}

// ---------------------------------------------------------------------------
// FakeDb — in-memory canonical tables (integration_inbox, interaction,
// person/identity) implementing the inbox receipt SQL + the intake repos.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

class FakeDb {
  inbox: Row[] = []
  interactions: Row[] = []
  persons: Row[] = []
  identities: Row[] = []
  tasks: Row[] = []
  seq = 0
  now = FIXED_NOW
  createPersonCalls = 0

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  seedPerson(personId: string, identities: Array<{ kind: string; value: string }>) {
    this.persons.push({ id: personId, archived_at: null })
    for (const identity of identities) {
      this.identities.push({
        identity_id: `${personId}-${identity.kind}-${identity.value}`,
        person_id: personId,
        kind: identity.kind,
        normalized_value: identity.value,
        archived: false,
      })
    }
  }

  // -- intake repositories (pure in-memory methods) -------------------------

  async findInteractionBySourceIdentity(
    sourceSystem: string,
    sourceExternalId: string,
  ): Promise<Interaction | null> {
    const row = this.interactions.find(
      (i) => i.source_system === sourceSystem && i.source_external_id === sourceExternalId,
    )
    if (!row) return null
    return {
      id: row.id,
      personId: row.person_id,
      channel: row.channel,
      eventType: row.event_type,
      direction: row.direction ?? undefined,
      occurredAt: row.occurred_at,
      title: row.title ?? undefined,
      summary: row.summary ?? undefined,
      sourceSystem: row.source_system ?? undefined,
      sourceExternalId: row.source_external_id ?? undefined,
      sourceMetadata: row.source_metadata ?? {},
      createdAt: row.created_at,
    }
  }

  async personExists(personId: string): Promise<boolean> {
    return this.persons.some((p) => p.id === personId && p.archived_at === null)
  }

  async findIdentityMatch(hint: NormalizedIdentityHint): Promise<IdentityMatch | null> {
    const row = this.identities.find(
      (i) =>
        i.kind === hint.kind &&
        i.normalized_value === hint.normalizedValue &&
        i.archived !== true,
    )
    return row
      ? {
          identityId: row.identity_id,
          personId: row.person_id,
          kind: row.kind,
          normalizedValue: row.normalized_value,
        }
      : null
  }

  async findIdentityOwnership(hint: NormalizedIdentityHint) {
    const row = this.identities.find(
      (i) => i.kind === hint.kind && i.normalized_value === hint.normalizedValue,
    )
    return row
      ? {
          identityId: row.identity_id,
          personId: row.person_id,
          kind: row.kind,
          normalizedValue: row.normalized_value,
          archived: row.archived === true,
        }
      : null
  }

  async createPersonWithIdentities(): Promise<void> {
    // allowCreation is NEVER enabled for Mac-observer intake; a call here is
    // a test failure (counted, never executed in practice).
    this.createPersonCalls += 1
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

  // -- canonical interaction insert (mirrors db/interactions.createInteraction) --

  async createInteraction(input: {
    personId: string
    propertyId?: string
    dealId?: string
    channel: string
    eventType: string
    direction?: string
    occurredAt: string | Date
    title?: string
    summary?: string
    sourceSystem?: string
    sourceExternalId?: string
    sourceMetadata?: Record<string, unknown>
  }): Promise<{ interactionId: string; created: boolean }> {
    const sourceSystem = input.sourceSystem?.trim() || null
    const sourceExternalId = input.sourceExternalId?.trim() || null
    const existing = this.interactions.find(
      (i) => i.source_system === sourceSystem && i.source_external_id === sourceExternalId,
    )
    if (existing) return { interactionId: existing.id, created: false }
    const id = `interaction-${++this.seq}`
    this.interactions.push({
      id,
      person_id: input.personId,
      property_id: input.propertyId ?? null,
      deal_id: input.dealId ?? null,
      channel: input.channel,
      event_type: input.eventType,
      direction: input.direction ?? null,
      occurred_at:
        input.occurredAt instanceof Date
          ? input.occurredAt.toISOString()
          : input.occurredAt,
      title: input.title ?? null,
      summary: input.summary ?? null,
      source_system: sourceSystem,
      source_external_id: sourceExternalId,
      source_metadata: input.sourceMetadata ?? {},
      created_at: this.now,
    })
    return { interactionId: id, created: true }
  }

  // -- integration_inbox repository SQL ---------------------------------------

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''),
        '',
      ),
    )
    const p = params as any[]

    if (t.includes('insert into integration_inbox')) {
      const [source, sourceAccount, externalEventId] = p
      const existing = this.inbox.find(
        (r) =>
          r.source === source &&
          r.source_account === sourceAccount &&
          r.external_event_id === externalEventId,
      )
      if (existing) return Promise.resolve([]) // on conflict ... do nothing
      const row = {
        id: `inbox-${++this.seq}`,
        source,
        source_account: sourceAccount,
        external_event_id: externalEventId,
        event_type: p[3],
        occurred_at: p[4],
        observed_at: p[5],
        direction: p[6],
        correlation_id: p[7],
        thread_id: p[8],
        subject: p[9],
        summary: p[10],
        content_reference: p[11],
        provenance_reference: p[12],
        participant_identities: JSON.parse(p[13] ?? '[]'),
        contact_candidates: p[14] ? JSON.parse(p[14]) : null,
        attachment_metadata: p[15] ? JSON.parse(p[15]) : null,
        status: 'received',
        attempt_count: 0,
        max_attempts: p[16],
        last_error: null,
        processing_started_at: null,
        processing_completed_at: null,
        resolved_person_id: null,
        interaction_id: null,
        created_at: this.now,
        updated_at: this.now,
      }
      this.inbox.push(row)
      return Promise.resolve([{ ...row }])
    }
    if (t.includes('update integration_inbox') && t.includes('processing_started_at = now()')) {
      // claim: received -> processing, or re-claim a stale processing claim
      const [receiptId] = p
      const row = this.inbox.find((r) => r.id === receiptId)
      if (!row) return Promise.resolve([])
      const stale =
        row.status === 'processing' &&
        new Date(row.processing_started_at).getTime() <=
          new Date(this.now).getTime() - 15 * 60 * 1000
      if (row.status !== 'received' && !stale) return Promise.resolve([])
      row.status = 'processing'
      row.processing_started_at = this.now
      row.updated_at = this.now
      return Promise.resolve([{ ...row }])
    }
    if (t.includes('update integration_inbox') && t.includes('set status = case')) {
      // failReceipt: retry (received, attempt+1) or poison (attempts exhausted)
      // Params: [attempts, maxAttempts, attempts, error, attempts, maxAttempts,
      //         receiptId, claimToken] (attempts/maxAttempts repeat in the
      //         status and completion CASE expressions).
      const [attempts, maxAttempts, , error, , , receiptId, claimToken] = p
      const row = this.inbox.find((r) => r.id === receiptId)
      if (!row || row.status !== 'processing' || row.processing_started_at !== claimToken) {
        return Promise.resolve([])
      }
      row.attempt_count = attempts
      row.last_error = error
      row.status = attempts >= maxAttempts ? 'poisoned' : 'received'
      row.processing_started_at = null
      row.processing_completed_at = attempts >= maxAttempts ? this.now : null
      row.updated_at = this.now
      return Promise.resolve([{ ...row }])
    }
    if (t.includes('update integration_inbox') && t.includes('processing_started_at = null')) {
      // transition: processing -> terminal, guarded by the claim token
      const [to, interactionId, resolvedPersonId, receiptId, from, claimToken] = p
      const row = this.inbox.find((r) => r.id === receiptId)
      if (!row || row.status !== from || row.processing_started_at !== claimToken) {
        return Promise.resolve([])
      }
      row.status = to
      row.processing_started_at = null
      row.processing_completed_at = this.now
      row.interaction_id = interactionId ?? null
      row.resolved_person_id = resolvedPersonId ?? null
      row.updated_at = this.now
      return Promise.resolve([{ id: row.id }])
    }
    if (
      t.includes('select') &&
      t.includes('from integration_inbox') &&
      t.includes("where status = 'received'")
    ) {
      const [limit] = p
      return Promise.resolve(
        this.inbox
          .filter((r) => r.status === 'received')
          .sort((a, b) => (a.created_at < b.created_at ? -1 : 1))
          .slice(0, limit)
          .map((r) => ({ ...r })),
      )
    }
    if (
      t.includes('select') &&
      t.includes('from integration_inbox') &&
      t.includes("where status = 'poisoned'")
    ) {
      const [limit] = p
      return Promise.resolve(
        this.inbox
          .filter((r) => r.status === 'poisoned')
          .sort((a, b) => (a.updated_at > b.updated_at ? -1 : 1))
          .slice(0, limit)
          .map((r) => ({ ...r })),
      )
    }
    if (t.includes('select') && t.includes('from integration_inbox')) {
      const [source, sourceAccount, externalEventId] = p
      const row = this.inbox.find(
        (r) =>
          r.source === source &&
          r.source_account === sourceAccount &&
          r.external_event_id === externalEventId,
      )
      return Promise.resolve(row ? [{ ...row }] : [])
    }
    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }

  durability(persistInteraction = (input: any) => this.createInteraction(input)): IntegrationInboxDurability {
    return createIntegrationInboxDurability(this.tx, persistInteraction)
  }
}

function baseProcessInput(db: FakeDb, event: ExternalActivityEvent) {
  return {
    event,
    configuration: {
      ...DEFAULT_INTEGRATION_INBOX_CONFIGURATION,
      capabilities: {
        contacts: contactsCapability,
        calendar: calendarCapability,
        mail: mailCapability,
        messages: messagesCapability,
        whatsapp: whatsappCapability,
      },
    },
    repositories: db.repositories(),
    durability: db.durability(),
    channels: channelConfigurations,
    now: () => FIXED_NOW,
  }
}

// ---------------------------------------------------------------------------
// 1. Common observer contract (criteria 1 + 2)
// ---------------------------------------------------------------------------

test('observer contract: all five sources lower to the SAME neutral event surface', () => {
  const contacts = contactsEvent()
  const calendar = calendarEvent()
  const mail = mailEvent()
  const messages = lowered({
    source: 'messages',
    sourceAccount: 'iMessage:acct',
    rawEventId: 'chat-1',
    observedAt: '2026-08-22T10:20:00.000Z',
    payload: {
      eventType: 'messages.message_received',
      messageId: 'chat-1',
      occurredAt: '2026-08-22T10:19:00.000Z',
      conversationId: 'conv-1',
      from: { kind: 'phone', value: '+15550100001', displayName: 'Buyer 1' },
      to: [{ kind: 'phone', value: '+15550100000' }],
      summary: 'Are you available Thursday?',
      rawReference: 'messages-raw-1.json',
    },
  })
  const whatsapp = lowered({
    source: 'whatsapp',
    sourceAccount: 'WhatsApp:acct',
    rawEventId: 'wa-1',
    observedAt: '2026-08-22T10:25:00.000Z',
    payload: {
      eventType: 'whatsapp.message_received',
      messageId: 'wa-1',
      occurredAt: '2026-08-22T10:24:00.000Z',
      from: { kind: 'phone', value: '+15550100001' },
      to: [{ kind: 'phone', value: '+15550100000' }],
      summary: 'Hello',
      rawReference: 'whatsapp-raw-1.json',
    },
  })

  const events = [contacts, calendar, mail, messages, whatsapp]
  // Every event has the SAME neutral top-level shape — no source-specific
  // field leaks into CRM domain services (criterion 1).
  const expectedKeys = [
    'schemaVersion', 'source', 'sourceAccount', 'externalEventId', 'eventType',
    'occurredAt', 'observedAt', 'direction', 'participants', 'contactCandidates',
    'thread', 'content', 'attachments', 'correlationId', 'context', 'provenance',
  ].sort()
  for (const event of events) {
    assert.deepEqual(Object.keys(event).sort(), expectedKeys, event.source)
  }

  // Criterion 2: stable identity, occurredAt, source, participants, thread,
  // provenance + correlation metadata on every event.
  for (const event of events) {
    assert.equal(event.schemaVersion, 1)
    assert.ok(event.sourceAccount.length > 0, 'sourceAccount')
    assert.ok(event.externalEventId.length > 0, 'externalEventId')
    assert.ok(!Number.isNaN(new Date(event.occurredAt).getTime()), 'occurredAt')
    assert.ok(!Number.isNaN(new Date(event.observedAt).getTime()), 'observedAt')
    assert.ok(event.participants.length > 0, 'participants')
    assert.equal(event.provenance.adapterVersion, `${event.source}.v1`)
    assert.ok(event.provenance.rawReference, 'provenance rawReference')
  }

  assert.equal(contacts.contactCandidates?.length, 2)
  assert.equal(calendar.thread?.id, 'series-1')
  assert.equal(mail.thread?.inReplyTo, 'prev-1')
  assert.equal(mail.attachments?.[0]?.referenceId, 'att-1')
  assert.equal(messages.thread?.conversationId, 'conv-1')
  assert.equal(mail.correlationId, 'corr-mail-1')
})

test('capabilities are honest: available / unproven / unsupported (criterion 8)', () => {
  assert.equal(contactsCapability.status, 'available')
  assert.equal(calendarCapability.status, 'available')
  assert.equal(mailCapability.status, 'unproven')
  assert.equal(messagesCapability.status, 'unsupported')
  assert.equal(whatsappCapability.status, 'unsupported')
  assert.match(messagesCapability.reason, /no public macOS API|SIP/i)
  assert.ok(messagesCapability.requiredAccess.length > 0, 'requiredAccess documented')
})

test('MacIntegrationObserver.acquire only emits available sources', async () => {
  const observer = new MacIntegrationObserver([
    createFakeMacObserver([contactsFixture('1')]),
    createFakeMacObserver([calendarFixture('1')]),
    createFakeMacObserver([mailFixture('1')]), // unproven by default
    createFakeMacObserver([{
      source: 'messages',
      sourceAccount: 'iMessage:acct',
      rawEventId: 'chat-1',
      observedAt: '2026-08-22T10:20:00.000Z',
      payload: { eventType: 'messages.message_received', messageId: 'chat-1' },
    }]),
  ])
  const events = await observer.acquire()
  assert.deepEqual(
    events.map((e) => e.source).sort(),
    ['calendar', 'contacts'],
    'only available observers contribute facts',
  )
})

// ---------------------------------------------------------------------------
// 2. Inbox durability: dedupe, replay, claim, bounded retry, poison
//    (criteria 3 + 9 + migration 044)
// ---------------------------------------------------------------------------

test('migration 044: integration_inbox carries unique source identity, status vocabulary, bounded retry, references', () => {
  const migration = readFileSync(
    join(__dirname, '../../db/migrations/044_integration_inbox.sql'),
    'utf8',
  )
  assert.match(migration, /create table if not exists integration_inbox/)
  assert.match(
    migration,
    /constraint integration_inbox_source_identity_unique\s+unique \(source, source_account, external_event_id\)/,
  )
  for (const status of [
    'received', 'processing', 'completed', 'rejected',
    'resolution_required', 'duplicate', 'poisoned',
  ]) {
    assert.match(migration, new RegExp(`'${status}'`), `status vocabulary includes ${status}`)
  }
  assert.match(migration, /attempt_count integer/)
  assert.match(migration, /max_attempts integer/)
  assert.match(migration, /content_reference text/)
  assert.match(migration, /provenance_reference text/)
  assert.match(migration, /participant_identities jsonb/)
  assert.match(migration, /resolved_person_id uuid/)
  assert.match(migration, /constraint integration_inbox_attempts check/)
})

test('inbox: insert-or-read dedupe + claim/transition lifecycle', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', [{ kind: 'email', value: 'buyer1@example.com' }])
  const durability = db.durability()

  const first = await durability.insertOrReadReceipt({
    source: 'calendar',
    sourceAccount: 'iCloud:acct',
    externalEventId: 'EKEvent-1',
    eventType: 'calendar.event_created',
    occurredAt: '2026-08-22T14:00:00.000Z',
    observedAt: '2026-08-22T10:10:00.000Z',
    direction: 'inbound',
    correlationId: null,
    threadId: 'series-1',
    subject: 'Property viewing',
    summary: null,
    contentReference: null,
    provenanceReference: 'calendar-raw-1.json',
    participantIdentities: [{ kind: 'email', value: 'buyer1@example.com' }],
    contactCandidates: null,
    attachmentMetadata: null,
    maxAttempts: 3,
  })
  assert.equal(first.created, true)
  assert.equal(first.record.status, 'received')

  // Replay of the same source identity reads back the SAME receipt.
  const replay = await durability.insertOrReadReceipt({
    source: 'calendar',
    sourceAccount: 'iCloud:acct',
    externalEventId: 'EKEvent-1',
    eventType: 'calendar.event_created',
    occurredAt: '2026-08-22T14:00:00.000Z',
    observedAt: '2026-08-22T10:10:00.000Z',
    direction: 'inbound',
    correlationId: null,
    threadId: 'series-1',
    subject: 'Property viewing',
    summary: null,
    contentReference: null,
    provenanceReference: 'calendar-raw-1.json',
    participantIdentities: [{ kind: 'email', value: 'buyer1@example.com' }],
    contactCandidates: null,
    attachmentMetadata: null,
    maxAttempts: 3,
  })
  assert.equal(replay.created, false)
  assert.equal(replay.record.id, first.record.id)

  const claimed = await durability.claimReceipt(first.record.id)
  assert.equal(claimed?.status, 'processing')
  assert.ok(claimed?.processingStartedAt)

  // A fresh in-flight claim cannot be claimed twice.
  assert.equal(await durability.claimReceipt(first.record.id), null)

  const transitioned = await durability.transitionReceipt({
    receiptId: first.record.id,
    claimToken: claimed!.processingStartedAt as string,
    from: 'processing',
    to: 'completed',
    interactionId: 'interaction-1',
    resolvedPersonId: 'person-1',
  })
  assert.equal(transitioned, true)
  assert.equal(db.inbox[0].status, 'completed')
})

test('inbox: failReceipt bounds retry, then poisons (dead-letter) without touching others', async () => {
  const db = new FakeDb()
  const durability = db.durability()
  const { record } = await durability.insertOrReadReceipt({
    source: 'contacts',
    sourceAccount: 'iCloud:acct',
    externalEventId: 'contact-1',
    eventType: 'contact.updated',
    occurredAt: '2026-08-22T10:04:00.000Z',
    observedAt: '2026-08-22T10:05:00.000Z',
    direction: null,
    correlationId: null,
    threadId: null,
    subject: 'Buyer 1',
    summary: null,
    contentReference: null,
    provenanceReference: 'contacts-raw-1.json',
    participantIdentities: [],
    contactCandidates: null,
    attachmentMetadata: null,
    maxAttempts: 2,
  })
  const claim1 = await durability.claimReceipt(record.id)
  const retry = await durability.failReceipt({
    receiptId: record.id,
    claimToken: claim1!.processingStartedAt as string,
    error: 'boom 1',
    attempts: 1,
    maxAttempts: 2,
  })
  assert.equal(retry?.status, 'received')
  assert.equal(retry?.attemptCount, 1)
  assert.equal(retry?.lastError, 'boom 1')

  const claim2 = await durability.claimReceipt(record.id)
  const poisoned = await durability.failReceipt({
    receiptId: record.id,
    claimToken: claim2!.processingStartedAt as string,
    error: 'boom 2',
    attempts: 2,
    maxAttempts: 2,
  })
  assert.equal(poisoned?.status, 'poisoned')
  assert.equal(poisoned?.attemptCount, 2)

  const escalated = await durability.listPoisoned(10)
  assert.equal(escalated.length, 1)
  assert.equal(escalated[0].id, record.id)
  assert.equal(escalated[0].lastError, 'boom 2')
})

// ---------------------------------------------------------------------------
// 3. End-to-end processing through the existing intake stubs
//    (criteria 3, 4, 5, 6)
// ---------------------------------------------------------------------------

test('calendar event: completed interaction via the existing intake stub; replay dedupes', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', [
    { kind: 'email', value: 'buyer1@example.com' },
    { kind: 'phone', value: '+15550100001' },
  ])
  const event = calendarEvent()

  const first = await processExternalActivityEvent(baseProcessInput(db, event))
  assert.equal(first.outcome, 'completed')
  if (first.outcome !== 'completed') return
  assert.equal(first.created, true)
  assert.equal(first.resolvedPersonId, 'person-1')
  assert.ok(first.interactionId)

  const interaction = db.interactions.find((i) => i.id === first.interactionId)
  assert.equal(interaction.channel, 'calendar')
  assert.equal(interaction.event_type, 'appointment')
  assert.equal(interaction.person_id, 'person-1')
  assert.equal(interaction.source_system, 'calendar:macos:icloud-acct')
  assert.equal(interaction.source_external_id, 'EKEvent-1')
  assert.equal(interaction.title, 'Property viewing')

  // No person was auto-created and no task/deal noise was derived.
  assert.equal(db.createPersonCalls, 0, 'no person auto-creation from an observed event')
  assert.equal(db.tasks.length, 0, 'no follow-up task noise')

  // Replay of the same external event: same receipt, same interaction, no
  // duplicate write (criterion 3).
  const replay = await processExternalActivityEvent(baseProcessInput(db, event))
  assert.equal(replay.outcome, 'completed')
  if (replay.outcome !== 'completed') return
  assert.equal(replay.created, false)
  assert.equal(replay.interactionId, first.interactionId)
  assert.equal(db.inbox.length, 1, 'one durable receipt')
  assert.equal(
    db.interactions.filter((i) => i.source_external_id === 'EKEvent-1').length,
    1,
    'one canonical interaction',
  )
})

test('mail event: completed email interaction via the existing email intake stub', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', [{ kind: 'email', value: 'buyer1@example.com' }])
  const event = mailEvent()

  // Mail is 'unproven' by default — an explicit fake-for-test override lets
  // the contract be exercised end-to-end without pretending real access.
  const input = baseProcessInput(db, event)
  input.configuration.capabilities.mail = { status: 'available', reason: 'fake-for-test', requiredAccess: [], supportedAppleFrameworks: [] }

  const result = await processExternalActivityEvent(input)
  assert.equal(result.outcome, 'completed')
  if (result.outcome !== 'completed') return
  const interaction = db.interactions.find((i) => i.id === result.interactionId)
  assert.equal(interaction.channel, 'email')
  assert.equal(interaction.event_type, 'email_received')
  assert.equal(interaction.direction, 'inbound')
  assert.equal(interaction.source_system, 'email:macos:imap-acct')
  assert.equal(interaction.source_external_id, 'msg-1')
  assert.equal(interaction.title, 'Property inquiry')
  assert.deepEqual(interaction.source_metadata.attachments, [
    { providerAttachmentId: 'att-1', filename: 'plans.pdf', mimeType: 'application/pdf', sizeBytes: 2048 },
  ])
  assert.equal(db.tasks.length, 0)
  assert.equal(db.createPersonCalls, 0)
})

test('contacts event: identity/contact resolution onto the canonical person BEFORE any mutation (criterion 5)', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', [
    { kind: 'email', value: 'buyer1@example.com' },
    { kind: 'phone', value: '+15550100001' },
  ])
  const result = await processExternalActivityEvent(baseProcessInput(db, contactsEvent('1')))
  assert.equal(result.outcome, 'completed')
  if (result.outcome !== 'completed') return
  assert.equal(result.resolvedPersonId, 'person-1')
  // A pure address-book fact converges on the person spine — no interaction
  // row, no person creation, no task.
  assert.equal(result.interactionId, undefined)
  assert.equal(db.interactions.length, 0)
  assert.equal(db.createPersonCalls, 0)
  assert.equal(db.tasks.length, 0)
  // The receipt records the convergence.
  assert.equal(db.inbox[0].resolved_person_id, 'person-1')
  assert.equal(db.inbox[0].status, 'completed')
})

test('contacts event with unknown identity: resolution_required (HumanRequired), never auto-creates', async () => {
  const db = new FakeDb() // NO persons seeded
  const result = await processExternalActivityEvent(baseProcessInput(db, contactsEvent('9')))
  assert.equal(result.outcome, 'resolution_required')
  assert.equal(db.interactions.length, 0)
  assert.equal(db.createPersonCalls, 0, 'observed address-book data never authorizes creation')
  assert.equal(db.inbox[0].status, 'resolution_required')
})

test('unsupported sources are skipped honestly: no receipt, no interaction (criterion 8)', async () => {
  const db = new FakeDb()
  for (const source of ['messages', 'whatsapp'] as const) {
    const event = lowered({
      source,
      sourceAccount: `${source}:acct`,
      rawEventId: `${source}-1`,
      observedAt: '2026-08-22T10:20:00.000Z',
      payload: {
        eventType: `${source}.message_received`,
        messageId: `${source}-1`,
        occurredAt: '2026-08-22T10:19:00.000Z',
        from: { kind: 'phone', value: '+15550100001' },
        to: [{ kind: 'phone', value: '+15550100000' }],
        summary: 'Hello',
      },
    })
    const result = await processExternalActivityEvent(baseProcessInput(db, event))
    assert.equal(result.outcome, 'skipped_unsupported', source)
    if (result.outcome === 'skipped_unsupported') {
      assert.match(result.reason, /unsupported/)
    }
    assert.equal(db.inbox.length, 0, `${source} never reaches the inbox`)
    assert.equal(db.interactions.length, 0)
  }
})

test('poison isolation: a failing event poisons without blocking other intake (criterion 9)', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', [{ kind: 'email', value: 'buyer1@example.com' }])

  // A calendar event with NO attendees throws in the mapper deterministically.
  const poisonEvent = lowered({
    source: 'calendar',
    sourceAccount: 'iCloud:acct',
    rawEventId: 'EKEvent-BAD',
    observedAt: '2026-08-22T10:10:00.000Z',
    payload: {
      eventType: 'calendar.event_created',
      calendarEventId: 'EKEvent-BAD',
      changedAt: '2026-08-22T10:09:00.000Z',
      eventStartAt: '2026-08-22T14:00:00.000Z',
      organizer: 'external',
      attendees: [],
      title: 'Broken',
    },
  })

  const attempts: string[] = []
  for (let i = 1; i <= 3; i += 1) {
    const result = await processExternalActivityEvent(baseProcessInput(db, poisonEvent))
    attempts.push(result.outcome)
  }
  assert.deepEqual(attempts, ['failed_retryable', 'failed_retryable', 'poisoned'])
  assert.equal(db.inbox[0].status, 'poisoned')
  assert.equal(db.inbox[0].attempt_count, 3)
  assert.match(db.inbox[0].last_error, /no attendee identities/)

  // A healthy event still completes — the poisoned receipt never blocks it.
  const healthy = await processExternalActivityEvent(baseProcessInput(db, calendarEvent()))
  assert.equal(healthy.outcome, 'completed')
  assert.equal(db.interactions.length, 1)
  assert.equal(db.inbox.length, 2)
  assert.equal(db.inbox.filter((r) => r.status === 'poisoned').length, 1)
})

test('in-flight: a fresh processing claim returns in_flight and does no work', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', [{ kind: 'email', value: 'buyer1@example.com' }])
  // Seed a receipt that is ALREADY processing (another worker owns it).
  db.inbox.push({
    id: 'inbox-inflight',
    source: 'calendar',
    source_account: 'iCloud:acct',
    external_event_id: 'EKEvent-1',
    event_type: 'calendar.event_created',
    occurred_at: '2026-08-22T14:00:00.000Z',
    observed_at: '2026-08-22T10:10:00.000Z',
    direction: 'inbound',
    correlation_id: null,
    thread_id: null,
    subject: null,
    summary: null,
    content_reference: null,
    provenance_reference: null,
    participant_identities: [],
    contact_candidates: null,
    attachment_metadata: null,
    status: 'processing',
    attempt_count: 0,
    max_attempts: 3,
    last_error: null,
    processing_started_at: FIXED_NOW,
    processing_completed_at: null,
    resolved_person_id: null,
    interaction_id: null,
    created_at: FIXED_NOW,
    updated_at: FIXED_NOW,
  })
  const result = await processExternalActivityEvent(baseProcessInput(db, calendarEvent()))
  assert.equal(result.outcome, 'in_flight')
  assert.equal(db.interactions.length, 0, 'no work happens for an in-flight receipt')
})

// ---------------------------------------------------------------------------
// 4. Acquisition-only + full sync loop (criterion 4)
// ---------------------------------------------------------------------------

test('syncMacObservations: the observer is acquisition-only — interactions only, never tasks/deals/workflows', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', [
    { kind: 'email', value: 'buyer1@example.com' },
    { kind: 'phone', value: '+15550100001' },
  ])
  const observer = new MacIntegrationObserver([
    createFakeMacObserver([contactsFixture('1')]),
    createFakeMacObserver([calendarFixture('1')]),
    createFakeMacObserver([mailFixture('1')]), // unproven — contributes nothing
  ])
  const capabilities = createMacCapabilities(observer.sourceObservers)

  const result = await syncMacObservations({
    acquire: () => observer.acquire(),
    configuration: { ...DEFAULT_INTEGRATION_INBOX_CONFIGURATION, capabilities },
    repositories: db.repositories(),
    durability: db.durability(),
    channels: channelConfigurations,
    now: () => FIXED_NOW,
  })

  assert.deepEqual(result.events.map((e) => e.source).sort(), ['calendar', 'contacts'])
  const completed = result.outcomes.filter((o) => o.outcome === 'completed')
  assert.equal(completed.length, 2)

  // One canonical interaction (calendar); contacts converge on the person.
  assert.equal(db.interactions.length, 1)
  assert.equal(db.interactions[0].channel, 'calendar')
  assert.equal(db.createPersonCalls, 0)
  assert.equal(db.tasks.length, 0, 'observer never creates tasks')
  // Receipts: calendar completed with interaction; contacts completed with person.
  assert.equal(db.inbox.length, 2)
  const contactsReceipt = db.inbox.find((r) => r.source === 'contacts')
  assert.equal(contactsReceipt.status, 'completed')
  assert.equal(contactsReceipt.resolved_person_id, 'person-1')
  assert.equal(contactsReceipt.interaction_id, null)
})

// ---------------------------------------------------------------------------
// 5. Canonical Business Command layer (criterion 7)
// ---------------------------------------------------------------------------

test('interaction.record: canonical command with claim-first receipt + INTERACTION_RECORDED event', async () => {
  const db = new FakeDb()
  const receipts: Row[] = []
  const collector = new InMemoryDomainEventCollector()

  const tx: QueryExecutor = (strings, ...params) => {
    const t = strings.reduce((acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''), '').replace(/\s+/g, ' ').trim().toLowerCase()
    const p = params as any[]

    if (t.includes('insert into workflow_command_receipt') && t.includes('on conflict')) {
      if (receipts.some((r) => r.command_id === p[0])) return Promise.resolve([])
      receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null, actor_app_user_id: p[1] ?? null })
      return Promise.resolve([{ command_id: p[0] }])
    }
    if (t.includes('update workflow_command_receipt set outcome')) {
      const r = receipts.find((x) => x.command_id === p[4])
      if (r) {
        r.outcome = p[0]
        r.aggregate_id = p[1]
        r.message = p[2]
        r.actor_app_user_id = p[3] ?? null
      }
      return Promise.resolve([])
    }
    if (t.includes('select command_id, outcome, aggregate_id, message, actor_app_user_id') && t.includes('where command_id')) {
      const r = receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(r ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message, actor_app_user_id: r.actor_app_user_id ?? null }] : [])
    }
    if (t.includes('insert into interaction') && t.includes('on conflict')) {
      const [personId, propertyId, dealId, channel, eventType, direction, occurredAt, title, summary, durationSeconds, sourceSystem, sourceExternalId, sourceMetadata] = p
      const existing = db.interactions.find((i) => i.source_system === sourceSystem && i.source_external_id === sourceExternalId)
      if (existing) return Promise.resolve([])
      const id = `interaction-${++db.seq}`
      db.interactions.push({
        id, person_id: personId, property_id: propertyId ?? null, deal_id: dealId ?? null,
        channel, event_type: eventType, direction: direction ?? null, occurred_at: occurredAt,
        title: title ?? null, summary: summary ?? null, duration_seconds: durationSeconds ?? null,
        source_system: sourceSystem ?? null, source_external_id: sourceExternalId ?? null,
        source_metadata: sourceMetadata ?? {}, created_at: FIXED_NOW,
      })
      return Promise.resolve([{ id, person_id: personId, property_id: null, deal_id: null, channel, event_type: eventType, direction: direction ?? null, occurred_at: occurredAt, title: title ?? null, summary: summary ?? null, duration_seconds: null, source_system: sourceSystem ?? null, source_external_id: sourceExternalId ?? null, source_metadata: {}, created_at: FIXED_NOW }])
    }
    if (t.includes('select') && t.includes('from interaction') && t.includes('source_system')) {
      const [sourceSystem, sourceExternalId] = p
      const row = db.interactions.find((i) => i.source_system === sourceSystem && i.source_external_id === sourceExternalId)
      return Promise.resolve(row ? [{ id: row.id, person_id: row.person_id, property_id: null, deal_id: null, channel: row.channel, event_type: row.event_type, direction: row.direction ?? null, occurred_at: row.occurred_at, title: row.title ?? null, summary: row.summary ?? null, duration_seconds: null, source_system: row.source_system ?? null, source_external_id: row.source_external_id ?? null, source_metadata: row.source_metadata ?? {}, created_at: row.created_at }] : [])
    }
    throw new Error(`FAKE_CMD_UNHANDLED: ${t}`)
  }

  const registry = new InMemoryCommandRegistry()
  registry.register(INTERACTION_RECORD, new RecordInteractionCommand())
  assert.ok(registry.resolve(INTERACTION_RECORD), 'interaction.record is a canonical command')

  const envelope: CommandEnvelope = {
    commandId: 'integration-inbox:calendar:macos:icloud-acct:EKEvent-1',
    commandType: INTERACTION_RECORD,
    actorAppUserId: null,
    aggregateType: 'interaction',
    aggregateId: null,
    correlationId: 'corr-1',
    causationId: null,
    requestedAt: FIXED_NOW,
    input: {
      personId: 'person-1',
      channel: 'calendar',
      eventType: 'appointment',
      direction: 'inbound',
      occurredAt: '2026-08-22T14:00:00.000Z',
      title: 'Property viewing',
      sourceSystem: 'calendar:macos:icloud-acct',
      sourceExternalId: 'EKEvent-1',
      sourceMetadata: {},
    },
  }

  const ctx = {
    tx,
    receipts: {
      find: async (commandId: string) => {
        const r = receipts.find((x) => x.command_id === commandId && x.outcome !== 'pending')
        return r
          ? { commandId: r.command_id, outcome: r.outcome, status: r.outcome === 'success' ? 'Succeeded' : 'Failed', aggregateId: r.aggregate_id, message: r.message, createdAt: null, actorAppUserId: r.actor_app_user_id ?? null }
          : null
      },
      save: async (receipt: any, t: QueryExecutor) => {
        await t`update workflow_command_receipt set outcome = ${receipt.outcome}, aggregate_id = ${receipt.aggregateId}, message = ${receipt.message}, actor_app_user_id = ${receipt.actorAppUserId ?? null} where command_id = ${receipt.commandId}`
      },
      claim: async (commandId: string, t: QueryExecutor) => {
        const rows = await t`insert into workflow_command_receipt (command_id, outcome, aggregate_id, message, actor_app_user_id) values (${commandId}, 'pending', null, null, null) on conflict (command_id) do nothing returning command_id`
        return rows.length > 0
      },
    },
    registry,
    events: collector,
    run: (cb: (t: QueryExecutor) => Promise<unknown>) => cb(tx),
    now: () => new Date(FIXED_NOW),
  }

  const command = registry.resolve(INTERACTION_RECORD)!
  const result = await command.handle(envelope, ctx as any)

  assert.equal(result.outcome, 'success')
  assert.equal(result.aggregateId, db.interactions[0].id)
  const events = collector.drain()
  assert.equal(events.length, 1)
  assert.equal(events[0].eventType, 'INTERACTION_RECORDED')
  assert.equal(events[0].aggregateType, 'interaction')
  assert.equal(events[0].correlationId, 'corr-1')
  assert.equal((events[0].payload as { interactionId: string }).interactionId, db.interactions[0].id)
  assert.equal(receipts[0].outcome, 'success', 'claim-first receipt finalized')
})

test('wiring: createCommandSeamInteractionPersistence routes through interaction.record (criterion 7)', async () => {
  const db = new FakeDb()
  const { createCommandSeamInteractionPersistence } = await import('../../lib/integration-inbox/wiring')
  const registry = new InMemoryCommandRegistry()
  registry.register(INTERACTION_RECORD, new RecordInteractionCommand())

  // Minimal dispatcher double: executes the handler + receipt bookkeeping.
  const receipts: Row[] = []
  const tx: QueryExecutor = (strings, ...params) => {
    const t = strings.reduce((acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''), '').replace(/\s+/g, ' ').trim().toLowerCase()
    const p = params as any[]
    if (t.includes('insert into workflow_command_receipt') && t.includes('on conflict')) {
      if (receipts.some((r) => r.command_id === p[0])) return Promise.resolve([])
      receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null })
      return Promise.resolve([{ command_id: p[0] }])
    }
    if (t.includes('update workflow_command_receipt set outcome')) {
      const r = receipts.find((x) => x.command_id === p[4])
      if (r) { r.outcome = p[0]; r.aggregate_id = p[1]; r.message = p[2] }
      return Promise.resolve([])
    }
    if (t.includes('select command_id, outcome, aggregate_id, message') && t.includes('where command_id')) {
      const r = receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(r ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message }] : [])
    }
    if (t.includes('insert into interaction') && t.includes('on conflict')) {
      const existing = db.interactions.find((i) => i.source_system === p[10] && i.source_external_id === p[11])
      if (existing) return Promise.resolve([])
      const id = `interaction-${++db.seq}`
      db.interactions.push({ id, person_id: p[0], property_id: p[1] ?? null, deal_id: p[2] ?? null, channel: p[3], event_type: p[4], direction: p[5] ?? null, occurred_at: p[6], title: p[7] ?? null, summary: p[8] ?? null, source_system: p[10] ?? null, source_external_id: p[11] ?? null, source_metadata: p[12] ?? {}, created_at: FIXED_NOW })
      return Promise.resolve([{ id, person_id: p[0], property_id: p[1] ?? null, deal_id: p[2] ?? null, channel: p[3], event_type: p[4], direction: p[5] ?? null, occurred_at: p[6], title: p[7] ?? null, summary: p[8] ?? null, duration_seconds: null, source_system: p[10] ?? null, source_external_id: p[11] ?? null, source_metadata: p[12] ?? {}, created_at: FIXED_NOW }])
    }
    throw new Error(`FAKE_WIRE_UNHANDLED: ${t}`)
  }
  const dispatcher = {
    execute: async (envelope: CommandEnvelope) => {
      assert.equal(envelope.commandType, INTERACTION_RECORD)
      const handler = registry.resolve(envelope.commandType)!
      const collector = new InMemoryDomainEventCollector()
      const ctx = {
        tx,
        receipts: {
          find: async (commandId: string) => {
            const r = receipts.find((x) => x.command_id === commandId && x.outcome !== 'pending')
            return r ? { commandId: r.command_id, outcome: r.outcome, aggregateId: r.aggregate_id, message: r.message } : null
          },
          save: async (receipt: any, t: QueryExecutor) => {
            await t`update workflow_command_receipt set outcome = ${receipt.outcome}, aggregate_id = ${receipt.aggregateId}, message = ${receipt.message}, actor_app_user_id = ${receipt.actorAppUserId ?? null} where command_id = ${receipt.commandId}`
          },
          claim: async (commandId: string, t: QueryExecutor) => {
            const rows = await t`insert into workflow_command_receipt (command_id, outcome, aggregate_id, message) values (${commandId}, 'pending', null, null) on conflict (command_id) do nothing returning command_id`
            return rows.length > 0
          },
        },
        registry,
        events: collector,
        run: (cb: (t: QueryExecutor) => Promise<unknown>) => cb(tx),
        now: () => new Date(FIXED_NOW),
      }
      const result = await handler.handle(envelope, ctx as any)
      return { ...result, replayed: false }
    },
  }

  const persist = createCommandSeamInteractionPersistence(dispatcher as any)
  const persisted = await persist({
    personId: 'person-1',
    channel: 'calendar',
    eventType: 'appointment',
    occurredAt: '2026-08-22T14:00:00.000Z',
    sourceSystem: 'calendar:macos:icloud-acct',
    sourceExternalId: 'EKEvent-1',
    sourceMetadata: {},
  })
  assert.ok(persisted.interactionId)
  assert.equal(persisted.created, true)
  assert.equal(db.interactions.length, 1)
  assert.equal(receipts[0].outcome, 'success')
})

// ---------------------------------------------------------------------------
// 6. Minimal retention (criterion 10)
// ---------------------------------------------------------------------------

test('inbox stores neutral references only — raw payloads never persist (criterion 10)', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', [{ kind: 'email', value: 'buyer1@example.com' }])
  const event = mailEvent()
  const input = baseProcessInput(db, event)
  input.configuration.capabilities.mail = { status: 'available', reason: 'fake-for-test', requiredAccess: [], supportedAppleFrameworks: [] }
  await processExternalActivityEvent(input)

  const receipt = db.inbox[0]
  // Neutral business facts ARE stored (participants, thread, references)...
  assert.equal(receipt.thread_id, 'thread-1')
  assert.equal(receipt.content_reference, 'imap-uid-1')
  assert.equal(receipt.provenance_reference, 'mail-raw-1.eml')
  assert.equal(receipt.summary, 'Interested in Casa Luar.')
  assert.equal(receipt.attachment_metadata[0].referenceId, 'att-1')
  assert.equal(receipt.participant_identities.length, 2)
  // ...but the raw payload/body is never duplicated onto the receipt row: the
  // row has exactly the neutral column surface (no 'payload' key, no secret
  // fields), and the canonical interaction stores normalized business data.
  assert.ok(!('payload' in receipt), 'no raw payload column on the receipt row')
  assert.ok(!('rawReference' in receipt), 'no rawReference passthrough on the receipt row')
  const serialized = JSON.stringify(receipt)
  assert.ok(!serialized.includes('accessToken') && !serialized.includes('password'))
  assert.equal(db.interactions[0].source_metadata.threadId, 'thread-1')
  assert.equal(db.interactions[0].source_metadata.attachments[0].providerAttachmentId, 'att-1')
  // The raw envelope artifact is only REFERENCED, and the event itself stays
  // out of canonical tables entirely.
  assert.equal(receipt.source, 'mail')
  assert.equal(receipt.external_event_id, 'msg-1')
})
