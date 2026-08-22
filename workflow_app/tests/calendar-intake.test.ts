import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import {
  createServer,
  type Server,
  type IncomingMessage,
  type ServerResponse,
} from 'node:http'
import { once } from 'node:events'

// ---------------------------------------------------------------------------
// CRM-08 — Calendar intake (live transport + durability layer).
//
// Scoped per the runtime test policy: this file only — no full regression, no
// persistence harness. It proves, against an in-memory FakeDb + an in-process
// FAKE Google Calendar server (real HTTP), that:
//   1. calendar_intake_receipt exists with UNIQUE (source_system,
//      source_external_id) idempotency (migration + repository contract);
//   2. the CalendarProvider interface + Google adapter own OAuth + a token
//      store isolated from canonical tables;
//   3. the lowering path persists the canonical interaction ONLY when ready
//      and advances the cursor;
//   4. replayed provider events dedupe to the same receipt — no duplicate
//      interaction;
//   5. no person is auto-created and no task is derived from a calendar event;
//   6. provider payloads/tokens never leak into canonical CRM rows;
//   7. Google push-notification verification fails closed on forgeries.
// ---------------------------------------------------------------------------

import { loadGoogleCalendarConfig } from '../../lib/calendar/google/config'
import { GoogleCalendarClient } from '../../lib/calendar/google/client'
import {
  GoogleCalendarProvider,
  lowerGoogleCalendarEvent,
} from '../../lib/calendar/google/adapter'
import { verifyGoogleCalendarWebhook } from '../../lib/calendar/google/webhook'
import { InMemoryCalendarTokenStore } from '../../lib/calendar/token-store'
import { createPostgresCalendarTokenStore } from '../../db/google-calendar-token'
import {
  processCalendarEvent,
  syncCalendarEvents,
  handleCalendarWebhook,
} from '../../lib/calendar/lowering'
import {
  createCalendarIntakeDurability,
  getCalendarIntakeReceiptBySourceIdentity,
  readCalendarIntakeCursor,
} from '../../db/calendar-intake-receipt'
import type { CalendarProvider } from '../../lib/calendar/contracts'
import type {
  CalendarAdapterConfiguration,
  CalendarProviderEvent,
} from '../../lib/crm-calendar-types'
import type { CalendarIntakeRepositories } from '../../lib/crm-calendar-intake'
import type { QueryExecutor } from '../../db/query-executor'
import type {
  IdentityMatch,
  NormalizedIdentityHint,
} from '../../lib/crm-intake-types'
import type { Interaction } from '../../lib/crm-types'

const FIXED_NOW = '2026-08-22T00:00:00.000Z'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function providerEvent(overrides: Partial<CalendarProviderEvent> = {}): CalendarProviderEvent {
  return {
    provider: 'stub',
    accountNamespace: 'acme',
    providerEventId: 'evt-1',
    occurredAt: '2026-08-20T14:00:00.000Z',
    organizer: 'external',
    attendees: [
      { kind: 'email', value: 'agent@culebraluxe.example' },
      { kind: 'email', value: 'buyer1@example.com' },
    ],
    actorAssurance: 'transport_observed',
    title: 'Property viewing',
    ...overrides,
  }
}

const configuration: CalendarAdapterConfiguration = {
  ownedCalendarEmails: [{ email: 'agent@culebraluxe.example' }],
}

class StubCalendarProvider implements CalendarProvider {
  readonly name = 'stub'
  readonly accountNamespace = 'acme'
  constructor(
    private readonly events: CalendarProviderEvent[],
    private readonly nextCursorValue: string | null = 'cursor-2',
  ) {}
  async listEventsSince(): Promise<{ events: CalendarProviderEvent[]; nextCursor: string | null }> {
    return { events: this.events, nextCursor: this.nextCursorValue }
  }
  async getEvent(id: string): Promise<CalendarProviderEvent | null> {
    return this.events.find((e) => e.providerEventId === id) ?? null
  }
  async verifyWebhook(): Promise<never> {
    throw new Error('unused')
  }
}

// ---------------------------------------------------------------------------
// FakeDb — in-memory canonical tables (receipt, interaction, person/identity,
// task) implementing the receipt repository SQL + the intake repositories.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

class FakeDb {
  receipts: Row[] = []
  interactions: Row[] = []
  persons: Row[] = []
  identities: Row[] = []
  tasks: Row[] = []
  tokens: Row[] = []
  seq = 0
  now = FIXED_NOW
  createPersonCalls = 0

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
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
    return this.persons.some((p) => p.id === personId)
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
    // allowCreation is NEVER enabled for calendar intake; a call here is a
    // test failure (counted, never executed in practice).
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

  repositories(): CalendarIntakeRepositories {
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

  // -- canonical interaction insert (mirrors db/interactions.createInteraction) -

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

  // -- receipt repository SQL -------------------------------------------------

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''),
        '',
      ),
    )
    const p = params as any[]

    // ---- calendar_intake_receipt ----
    if (t.includes('insert into calendar_intake_receipt')) {
      const [sourceSystem, sourceExternalId, providerCursor, syncedAt] = p
      const existing = this.receipts.find(
        (r) => r.source_system === sourceSystem && r.source_external_id === sourceExternalId,
      )
      if (existing) return Promise.resolve([]) // on conflict ... do nothing
      const row = {
        id: `receipt-${++this.seq}`,
        source_system: sourceSystem,
        source_external_id: sourceExternalId,
        status: 'received',
        interaction_id: null,
        provider_cursor: providerCursor ?? null,
        last_sync_at: syncedAt ?? null,
        processing_started_at: null,
        created_at: this.now,
        updated_at: this.now,
      }
      this.receipts.push(row)
      return Promise.resolve([{ ...row }])
    }
    if (t.includes('update calendar_intake_receipt') && t.includes('processing_started_at = now()')) {
      // claim: received -> processing, or re-claim a stale processing claim
      const [receiptId] = p
      const row = this.receipts.find((r) => r.id === receiptId)
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
    if (t.includes('update calendar_intake_receipt') && t.includes('processing_started_at = null')) {
      // transition: processing -> terminal, guarded by the claim token
      const [to, interactionId, receiptId, from, claimToken] = p
      const row = this.receipts.find((r) => r.id === receiptId)
      if (!row || row.status !== from || row.processing_started_at !== claimToken) {
        return Promise.resolve([])
      }
      row.status = to
      row.processing_started_at = null
      row.interaction_id = interactionId ?? null
      row.updated_at = this.now
      return Promise.resolve([{ id: row.id }])
    }
    if (t.includes('select provider_cursor from calendar_intake_receipt')) {
      const [sourceSystem] = p
      const cursorRow = this.receipts
        .filter((r) => r.source_system === sourceSystem && r.provider_cursor !== null)
        .sort((a, b) =>
          String(a.last_sync_at) < String(b.last_sync_at)
            ? 1
            : String(a.last_sync_at) > String(b.last_sync_at)
              ? -1
              : String(a.created_at) < String(b.created_at)
                ? 1
                : -1,
        )[0]
      return Promise.resolve(cursorRow ? [{ provider_cursor: cursorRow.provider_cursor }] : [])
    }
    if (t.includes('from calendar_intake_receipt')) {
      const [sourceSystem, sourceExternalId] = p
      const row = this.receipts.find(
        (r) => r.source_system === sourceSystem && r.source_external_id === sourceExternalId,
      )
      return Promise.resolve(row ? [{ ...row }] : [])
    }

    // ---- google_calendar_token_store (provider-side) ----
    if (t.includes('insert into google_calendar_token_store')) {
      const [accountNamespace, accessToken, expiresAt] = p
      const existing = this.tokens.find((r) => r.account_namespace === accountNamespace)
      if (existing) {
        existing.access_token = accessToken
        existing.access_token_expires_at = expiresAt
        existing.updated_at = this.now
        return Promise.resolve([])
      }
      this.tokens.push({
        account_namespace: accountNamespace,
        access_token: accessToken,
        access_token_expires_at: expiresAt,
        created_at: this.now,
        updated_at: this.now,
      })
      return Promise.resolve([])
    }
    if (t.includes('from google_calendar_token_store')) {
      const [accountNamespace] = p
      const row = this.tokens.find((r) => r.account_namespace === accountNamespace)
      return Promise.resolve(
        row
          ? [
              {
                access_token: row.access_token,
                access_token_expires_at: row.access_token_expires_at,
              },
            ]
          : [],
      )
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }

  seedPerson(personId: string, email: string) {
    this.persons.push({ id: personId, role: 'buyer' })
    this.identities.push({
      identity_id: `identity-${personId}`,
      person_id: personId,
      kind: 'email',
      normalized_value: email.toLowerCase(),
      archived: false,
    })
  }
}

// ---------------------------------------------------------------------------
// FakeGoogleCalendarServer — an in-process Google Calendar API + OAuth token
// endpoint for the adapter's real HTTP path.
// ---------------------------------------------------------------------------

type FakeGoogleEvent = {
  id: string
  status: string
  summary: string
  description?: string
  start: { dateTime: string }
  updated: string
  organizer: { email: string; self?: boolean }
  attendees?: Array<{ email: string; self?: boolean; responseStatus?: string }>
  iCalUID?: string
  hangoutLink?: string
  extendedProperties?: unknown
}

class FakeGoogleCalendarServer {
  static async start(): Promise<FakeGoogleCalendarServer> {
    const server = new FakeGoogleCalendarServer()
    server.httpServer = createServer((req, res) => void server.handle(req, res))
    server.httpServer.listen(0, '127.0.0.1')
    await once(server.httpServer, 'listening')
    const address = server.httpServer.address() as { port: number }
    server.baseUrl = `http://127.0.0.1:${address.port}`
    return server
  }

  readonly calendarId = 'primary'
  baseUrl = ''
  events = new Map<string, FakeGoogleEvent>()
  requests: Array<{ method: string; path: string; query: URLSearchParams; raw: string }> = []
  tokenSeq = 0
  syncTokenSeq = 0
  private httpServer: Server | null = null

  async stop(): Promise<void> {
    const server = this.httpServer
    if (!server) return
    server.closeAllConnections?.()
    server.close()
    await once(server, 'close').catch(() => undefined)
  }

  addEvent(overrides: Partial<FakeGoogleEvent> & { id: string }): FakeGoogleEvent {
    const event: FakeGoogleEvent = {
      id: overrides.id,
      status: 'confirmed',
      summary: `Appointment ${overrides.id}`,
      start: { dateTime: '2026-08-20T14:00:00.000Z' },
      updated: '2026-08-20T13:00:00.000Z',
      organizer: { email: 'agent@culebraluxe.example', self: true },
      attendees: [
        { email: 'buyer1@example.com', responseStatus: 'accepted' },
        { email: 'agent@culebraluxe.example', self: true },
      ],
      iCalUID: `uid-${overrides.id}@google.com`,
      hangoutLink: `https://meet.google.com/${overrides.id}`,
      ...overrides,
    }
    this.events.set(event.id, event)
    return event
  }

  requestCount(method: string, pathPrefix: string): number {
    return this.requests.filter((r) => r.method === method && r.path.startsWith(pathPrefix)).length
  }

  private async handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const chunks: Buffer[] = []
    for await (const chunk of req) chunks.push(chunk as Buffer)
    const raw = Buffer.concat(chunks).toString('utf8')
    const url = new URL(req.url ?? '/', this.baseUrl)
    this.requests.push({ method: req.method ?? '', path: url.pathname, query: url.searchParams, raw })

    // OAuth token refresh
    if (req.method === 'POST' && url.pathname === '/token') {
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(JSON.stringify({ access_token: `access-${++this.tokenSeq}`, expires_in: 3600 }))
      return
    }

    const eventsMatch = url.pathname.match(
      new RegExp(`^/calendar/v3/calendars/${this.calendarId}/events$`),
    )
    if (req.method === 'GET' && eventsMatch) {
      const updatedMin = url.searchParams.get('updatedMin')
      const timeMin = url.searchParams.get('timeMin')
      const syncToken = url.searchParams.get('syncToken')
      const items: FakeGoogleEvent[] = []
      for (const event of this.events.values()) {
        // Cancelled events are only visible via sync tokens (Google semantics).
        if (event.status === 'cancelled' && !syncToken) continue
        if (updatedMin && event.updated < updatedMin) continue
        if (timeMin && event.start.dateTime < timeMin) continue
        items.push(event)
      }
      const body: Record<string, unknown> = { items }
      // Google returns a nextSyncToken on every events.list response; the
      // adapter only consumes it in sync_token mode.
      body.nextSyncToken = `sync-${++this.syncTokenSeq}`
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(JSON.stringify(body))
      return
    }

    const singleMatch = url.pathname.match(
      new RegExp(`^/calendar/v3/calendars/${this.calendarId}/events/([^/]+)$`),
    )
    if (req.method === 'GET' && singleMatch) {
      const eventId = decodeURIComponent(singleMatch[1])
      const event = this.events.get(eventId)
      if (!event) {
        res.writeHead(404, { 'content-type': 'application/json' })
        res.end(JSON.stringify({ error: { code: 404, message: 'Not Found' } }))
        return
      }
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(JSON.stringify(event))
      return
    }

    res.writeHead(404, { 'content-type': 'application/json' })
    res.end(JSON.stringify({ error: { code: 404, message: `no route ${req.method} ${url.pathname}` } }))
  }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

function testGoogleConfig(overrides: Record<string, string> = {}) {
  return loadGoogleCalendarConfig({
    GOOGLE_CLIENT_ID: 'client-id',
    GOOGLE_CLIENT_SECRET: 'client-secret',
    GOOGLE_REFRESH_TOKEN: 'refresh-token',
    GOOGLE_CALENDAR_ID: 'primary',
    GOOGLE_CALENDAR_ACCOUNT_NAMESPACE: 'primary',
    GOOGLE_CALENDAR_SYNC_MODE: 'updated_time',
    GOOGLE_CALENDAR_WEBHOOK_CHANNEL_TOKEN: 'webhook-token',
    GOOGLE_CALENDAR_LOOKBACK_DAYS: '14',
    GOOGLE_CALENDAR_TIMEOUT_MS: '500',
    GOOGLE_CALENDAR_MAX_ATTEMPTS: '3',
    GOOGLE_CALENDAR_RETRY_BASE_DELAY_MS: '5',
    GOOGLE_CALENDAR_RETRY_MAX_DELAY_MS: '10',
    ...overrides,
  })
}

function makeGoogleProvider(server: FakeGoogleCalendarServer, overrides: Record<string, string> = {}) {
  const config = testGoogleConfig({
    GOOGLE_CALENDAR_TOKEN_ENDPOINT: `${server.baseUrl}/token`,
    GOOGLE_CALENDAR_API_BASE: `${server.baseUrl}/calendar/v3`,
    ...overrides,
  })
  const tokenStore = new InMemoryCalendarTokenStore()
  const client = new GoogleCalendarClient(config, {
    tokenStore,
    sleep: async () => {},
  })
  return {
    provider: new GoogleCalendarProvider(config, client),
    client,
    tokenStore,
  }
}

function googHeaders(overrides: Record<string, string> = {}) {
  return {
    'x-goog-channel-id': 'chan-1',
    'x-goog-resource-id': 'res-1',
    'x-goog-resource-state': 'updated',
    'x-goog-message-number': '3',
    'x-goog-channel-token': 'webhook-token',
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// 1. Receipt/cursor durability (real repository SQL against the FakeDb)
// ---------------------------------------------------------------------------

test('receipt: insert-or-read is idempotent on the unique source identity', async () => {
  const db = new FakeDb()
  const first = await createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
    .insertOrReadReceipt({
      sourceSystem: 'calendar:google:primary',
      sourceExternalId: 'evt-1',
      providerCursor: 'cursor-2',
      syncedAt: FIXED_NOW,
    })
  assert.equal(first.created, true)
  assert.equal(first.receipt.status, 'received')
  assert.equal(first.receipt.providerCursor, 'cursor-2')

  // replay of the same provider event: same receipt, nothing inserted
  const replay = await createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
    .insertOrReadReceipt({
      sourceSystem: 'calendar:google:primary',
      sourceExternalId: 'evt-1',
      providerCursor: 'cursor-2',
      syncedAt: FIXED_NOW,
    })
  assert.equal(replay.created, false)
  assert.equal(replay.receipt.id, first.receipt.id)
  assert.equal(db.receipts.length, 1, 'unique (source_system, source_external_id) — no second receipt')
})

test('receipt: claim + transition lifecycle enforces the claim token and completed state', async () => {
  const db = new FakeDb()
  const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))

  const { receipt } = await durability.insertOrReadReceipt({
    sourceSystem: 'calendar:google:primary',
    sourceExternalId: 'evt-1',
    providerCursor: null,
    syncedAt: FIXED_NOW,
  })
  assert.equal(receipt.status, 'received')

  const claimed = await durability.claimReceipt(receipt.id)
  assert.ok(claimed)
  assert.equal(claimed!.status, 'processing')
  assert.ok(claimed!.processingStartedAt)

  // a fresh in-flight claim is not claimable by another worker
  const secondClaim = await durability.claimReceipt(receipt.id)
  assert.equal(secondClaim, null)

  // terminal transition guarded by the claim token
  const wrongToken = await durability.transitionReceipt({
    receiptId: receipt.id,
    claimToken: 'not-the-token',
    from: 'processing',
    to: 'rejected',
  })
  assert.equal(wrongToken, false)

  const completed = await durability.transitionReceipt({
    receiptId: receipt.id,
    claimToken: claimed!.processingStartedAt as string,
    from: 'processing',
    to: 'completed',
    interactionId: 'interaction-1',
  })
  assert.equal(completed, true)

  const stored = await getCalendarIntakeReceiptBySourceIdentity(
    'calendar:google:primary',
    'evt-1',
    db.tx,
  )
  assert.equal(stored?.status, 'completed')
  assert.equal(stored?.interactionId, 'interaction-1')
  assert.equal(stored?.processingStartedAt, null)

  // a completed receipt is never re-claimed
  assert.equal(await durability.claimReceipt(receipt.id), null)

  // illegal transitions are rejected by the repository
  await assert.rejects(
    () =>
      durability.transitionReceipt({
        receiptId: receipt.id,
        claimToken: 'x',
        from: 'completed',
        to: 'duplicate',
      }),
    /transition is not allowed/,
  )
  await assert.rejects(
    () =>
      durability.transitionReceipt({
        receiptId: receipt.id,
        claimToken: 'x',
        from: 'processing',
        to: 'completed', // completed without interaction_id is invalid
      }),
    /Only a completed receipt may have an interaction ID/,
  )
})

test('receipt: a stale processing claim (crash between claim and transition) is re-claimable', async () => {
  const db = new FakeDb()
  const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
  const { receipt } = await durability.insertOrReadReceipt({
    sourceSystem: 'calendar:google:primary',
    sourceExternalId: 'evt-1',
    providerCursor: null,
    syncedAt: FIXED_NOW,
  })
  // simulate a crash: claimed two days ago, never transitioned
  const crashed = await durability.claimReceipt(receipt.id)
  assert.ok(crashed)
  const row = db.receipts.find((r) => r.id === receipt.id)!
  row.processing_started_at = '2026-08-20T00:00:00.000Z'

  const reClaimed = await durability.claimReceipt(receipt.id)
  assert.ok(reClaimed, 'stale processing claim is re-claimed after the window')
  assert.equal(reClaimed!.status, 'processing')
  assert.equal(reClaimed!.processingStartedAt, FIXED_NOW)
})

test('receipt: cursor read returns the most recently synced non-null cursor', async () => {
  const db = new FakeDb()
  const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
  await durability.insertOrReadReceipt({
    sourceSystem: 'calendar:google:primary',
    sourceExternalId: 'evt-1',
    providerCursor: 'cursor-2',
    syncedAt: '2026-08-21T00:00:00.000Z',
  })
  await durability.insertOrReadReceipt({
    sourceSystem: 'calendar:google:primary',
    sourceExternalId: 'evt-2',
    providerCursor: 'cursor-5',
    syncedAt: '2026-08-22T00:00:00.000Z',
  })
  // webhook-delivered receipt carries NO cursor and must not displace the poller's
  await durability.insertOrReadReceipt({
    sourceSystem: 'calendar:google:primary',
    sourceExternalId: 'evt-3',
    providerCursor: null,
    syncedAt: '2026-08-22T01:00:00.000Z',
  })

  assert.equal(await readCalendarIntakeCursor('calendar:google:primary', db.tx), 'cursor-5')
  assert.equal(await readCalendarIntakeCursor('calendar:google:other', db.tx), null)
})

test('migration 040: calendar_intake_receipt carries the unique source identity + status vocabulary', () => {
  const migration = readFileSync(
    join(__dirname, '../../db/migrations/040_calendar_intake_receipt.sql'),
    'utf8',
  )
  assert.match(migration, /create table if not exists calendar_intake_receipt/)
  assert.match(migration, /constraint calendar_intake_source_identity_unique\s+unique \(source_system, source_external_id\)/)
  for (const status of ['received', 'processing', 'completed', 'rejected', 'resolution_required', 'duplicate']) {
    assert.match(migration, new RegExp(`'${status}'`), `status vocabulary includes ${status}`)
  }
  assert.match(migration, /provider_cursor text/)
  assert.match(migration, /last_sync_at timestamptz/)
})

// ---------------------------------------------------------------------------
// 2. Config — credentials from env only, fail closed, never echoed
// ---------------------------------------------------------------------------

test('config: reads required keys from env with defaults; never echoes values', () => {
  const cfg = testGoogleConfig()
  assert.equal(cfg.clientId, 'client-id')
  assert.equal(cfg.calendarId, 'primary')
  assert.equal(cfg.accountNamespace, 'primary')
  assert.equal(cfg.syncMode, 'updated_time')
  assert.equal(cfg.lookbackDays, 14)
  assert.equal(cfg.webhookChannelToken, 'webhook-token')

  const missing = () =>
    loadGoogleCalendarConfig({
      GOOGLE_CLIENT_ID: 'c',
      GOOGLE_CALENDAR_ID: 'primary',
    })
  assert.throws(missing, /GOOGLE_CLIENT_SECRET, GOOGLE_REFRESH_TOKEN/)

  try {
    loadGoogleCalendarConfig({ GOOGLE_CLIENT_ID: 'super-secret-value' })
    assert.fail('expected loadGoogleCalendarConfig to throw')
  } catch (err) {
    assert.ok(!String(err).includes('super-secret-value'), 'error must never echo secret values')
  }

  assert.throws(
    () => testGoogleConfig({ GOOGLE_CALENDAR_SYNC_MODE: 'bogus' }),
    /must be 'updated_time' or 'sync_token'/,
  )
  // absent webhook channel token fails webhook verification closed
  assert.equal(testGoogleConfig({ GOOGLE_CALENDAR_WEBHOOK_CHANNEL_TOKEN: '' }).webhookChannelToken, null)
})

// ---------------------------------------------------------------------------
// 3. Webhook signature verification (Google push headers, constant-time)
// ---------------------------------------------------------------------------

test('webhook: valid channel token + known resource state verify; forgeries reject', () => {
  const token = 'webhook-token'
  assert.equal(verifyGoogleCalendarWebhook('', googHeaders(), token).resourceState, 'updated')
  assert.equal(verifyGoogleCalendarWebhook('', googHeaders({ 'x-goog-resource-state': 'sync' }), token).resourceState, 'sync')
  assert.equal(verifyGoogleCalendarWebhook('', googHeaders({ 'x-goog-resource-state': 'exists' }), token).resourceState, 'exists')
  assert.equal(verifyGoogleCalendarWebhook('', googHeaders({ 'x-goog-resource-state': 'deleted' }), token).resourceState, 'deleted')

  // wrong channel token
  assert.throws(
    () => verifyGoogleCalendarWebhook('', googHeaders({ 'x-goog-channel-token': 'forged' }), token),
    /channel token is invalid/,
  )
  // no configured token -> fail closed
  assert.throws(
    () => verifyGoogleCalendarWebhook('', googHeaders(), null),
    /not configured/,
  )
  // unknown resource state
  assert.throws(
    () => verifyGoogleCalendarWebhook('', googHeaders({ 'x-goog-resource-state': 'bogus' }), token),
    /invalid X-Goog-Resource-State/,
  )
  // missing channel id / resource id / message number
  assert.throws(
    () => verifyGoogleCalendarWebhook('', googHeaders({ 'x-goog-channel-id': '' }), token),
    /missing X-Goog-Channel-ID/,
  )
  assert.throws(
    () => verifyGoogleCalendarWebhook('', googHeaders({ 'x-goog-resource-id': '' }), token),
    /missing X-Goog-Resource-ID/,
  )
  assert.throws(
    () => verifyGoogleCalendarWebhook('', googHeaders({ 'x-goog-message-number': '0' }), token),
    /invalid X-Goog-Message-Number/,
  )
  // payload must be the raw body string
  assert.throws(
    () => verifyGoogleCalendarWebhook({ parsed: true }, googHeaders(), token),
    /raw request body string/,
  )
})

// ---------------------------------------------------------------------------
// 4. Google adapter — OAuth + token store + raw payload lowering
// ---------------------------------------------------------------------------

test('adapter: OAuth refresh happens once, token cached provider-side, reused across syncs', async () => {
  const db = new FakeDb()
  const server = await FakeGoogleCalendarServer.start()
  try {
    server.addEvent({ id: 'evt-1' })
    const { provider } = makeGoogleProvider(server)

    const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
    const first = await syncCalendarEvents({
      provider,
      configuration,
      repositories: db.repositories(),
      durability,
      now: () => FIXED_NOW,
    })
    assert.equal(first.outcomes.length, 1)
    assert.equal(first.outcomes[0].outcome, 'resolution_required', 'unknown attendee -> resolution_required (no person auto-created)')
    assert.equal(server.requestCount('POST', '/token'), 1, 'one OAuth refresh')

    const second = await syncCalendarEvents({
      provider,
      configuration,
      repositories: db.repositories(),
      durability,
      now: () => FIXED_NOW,
    })
    assert.equal(server.requestCount('POST', '/token'), 1, 'cached access token reused — no second refresh')
    assert.equal(second.outcomes.length, 0, 'incremental sync sees no changes after the cursor advance')
    assert.equal(db.receipts.length, 1, 'the advanced cursor prevents re-processing')
  } finally {
    await server.stop()
  }
})

test('adapter: lowers raw Google payloads — cancelled dropped, organizer.self -> owned, attendees mapped', () => {
  const cancelled = lowerGoogleCalendarEvent(
    { id: 'evt-x', status: 'cancelled', start: { dateTime: '2026-08-20T14:00:00Z' }, updated: '2026-08-20T13:00:00Z' },
    'primary',
  )
  assert.equal(cancelled, null)

  const lowered = lowerGoogleCalendarEvent(
    {
      id: 'evt-1',
      status: 'confirmed',
      summary: 'Site visit',
      description: 'Buyer viewing',
      start: { dateTime: '2026-08-20T14:00:00.000Z' },
      updated: '2026-08-20T13:00:00.000Z',
      organizer: { email: 'agent@culebraluxe.example', self: true },
      attendees: [
        { email: 'buyer1@example.com', responseStatus: 'accepted' },
        { email: 'agent@culebraluxe.example', self: true },
      ],
      iCalUID: 'uid-1@google.com',
      hangoutLink: 'https://meet.google.com/evt-1',
    },
    'primary',
  )
  assert.ok(lowered)
  assert.equal(lowered!.provider, 'google')
  assert.equal(lowered!.accountNamespace, 'primary')
  assert.equal(lowered!.providerEventId, 'evt-1')
  assert.equal(lowered!.organizer, 'owned')
  assert.deepEqual(lowered!.attendees, [
    { kind: 'email', value: 'buyer1@example.com' },
    { kind: 'email', value: 'agent@culebraluxe.example' },
  ])
  assert.equal(lowered!.actorAssurance, 'transport_observed')
  // raw Google fields NEVER cross the neutral event
  assert.ok(!JSON.stringify(lowered).includes('iCalUID'))
  assert.ok(!JSON.stringify(lowered).includes('hangoutLink'))
  assert.ok(!JSON.stringify(lowered).includes('responseStatus'))
})

test('adapter: listEventsSince advances an updated-time cursor and honors sync tokens', async () => {
  const server = await FakeGoogleCalendarServer.start()
  try {
    server.addEvent({ id: 'evt-1', updated: '2026-08-20T10:00:00.000Z', start: { dateTime: '2026-08-20T10:00:00.000Z' } })
    server.addEvent({ id: 'evt-2', updated: '2026-08-20T10:05:00.000Z', start: { dateTime: '2026-08-20T10:05:00.000Z' } })
    const { provider } = makeGoogleProvider(server)

    // updated_time mode: first sync (no cursor) -> both events; cursor advances
    const full = await provider.listEventsSince(null)
    assert.deepEqual(full.events.map((e) => e.providerEventId).sort(), ['evt-1', 'evt-2'])
    assert.equal(full.nextCursor, '2026-08-20T10:05:00.001Z', 'cursor = max(updated) + 1ms')

    // incremental sync with the advanced cursor -> nothing new
    const incremental = await provider.listEventsSince(full.nextCursor)
    assert.equal(incremental.events.length, 0)
    assert.equal(incremental.nextCursor, full.nextCursor, 'empty sync keeps the cursor')

    // a new event after the watermark is picked up
    server.addEvent({ id: 'evt-3', updated: '2026-08-20T11:00:00.000Z', start: { dateTime: '2026-08-20T11:00:00.000Z' } })
    const next = await provider.listEventsSince(full.nextCursor)
    assert.deepEqual(next.events.map((e) => e.providerEventId), ['evt-3'])
    assert.equal(next.nextCursor, '2026-08-20T11:00:00.001Z')

    // sync_token mode uses the provider's nextSyncToken
    const { provider: tokenProvider } = makeGoogleProvider(server, { GOOGLE_CALENDAR_SYNC_MODE: 'sync_token' })
    const tokenSync = await tokenProvider.listEventsSince(null)
    assert.ok(tokenSync.nextCursor?.startsWith('sync-'), 'sync token advanced from the provider response')
    const tokenSync2 = await tokenProvider.listEventsSince(tokenSync.nextCursor)
    assert.ok(tokenSync2.nextCursor?.startsWith('sync-'))
  } finally {
    await server.stop()
  }
})

test('adapter: getEvent lowers a single event and returns null for unknown ids', async () => {
  const server = await FakeGoogleCalendarServer.start()
  try {
    server.addEvent({ id: 'evt-1' })
    const { provider } = makeGoogleProvider(server)
    const event = await provider.getEvent('evt-1')
    assert.ok(event)
    assert.equal(event!.providerEventId, 'evt-1')
    assert.equal(await provider.getEvent('evt-unknown'), null)
  } finally {
    await server.stop()
  }
})

test('token store: postgres-backed store round-trips provider-side, isolated from canonical tables', async () => {
  const db = new FakeDb()
  const store = createPostgresCalendarTokenStore(db.tx)
  assert.equal(await store.getAccessToken('primary'), null)
  await store.setAccessToken('primary', 'access-abc', new Date('2026-08-22T01:00:00.000Z'))
  const cached = await store.getAccessToken('primary')
  assert.equal(cached?.accessToken, 'access-abc')
  assert.equal(db.tokens.length, 1)
  assert.equal(db.interactions.length, 0, 'tokens never touch canonical interaction rows')
  assert.equal(db.receipts.length, 0, 'tokens never touch canonical receipt rows')
  // upsert replaces the token
  await store.setAccessToken('primary', 'access-def', new Date('2026-08-22T02:00:00.000Z'))
  assert.equal((await store.getAccessToken('primary'))?.accessToken, 'access-def')
  assert.equal(db.tokens.length, 1)
})

// ---------------------------------------------------------------------------
// 5. Lowering — ready persists exactly one interaction; no person, no task
// ---------------------------------------------------------------------------

test('lowering: a ready event persists ONE canonical interaction and completes its receipt', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', 'buyer1@example.com')
  const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
  const provider = new StubCalendarProvider([providerEvent()], 'cursor-2')

  const sync = await syncCalendarEvents({
    provider,
    configuration,
    repositories: db.repositories(),
    durability,
    now: () => FIXED_NOW,
  })

  assert.equal(sync.outcomes.length, 1)
  const outcome = sync.outcomes[0]
  assert.equal(outcome.outcome, 'completed')
  assert.equal(outcome.created, true)
  assert.ok(outcome.interactionId)

  assert.equal(db.interactions.length, 1, 'exactly one canonical interaction')
  const interaction = db.interactions[0]
  assert.equal(interaction.channel, 'calendar')
  assert.equal(interaction.event_type, 'appointment')
  assert.equal(interaction.source_system, 'calendar:stub:acme')
  assert.equal(interaction.source_external_id, 'evt-1')
  assert.equal(interaction.person_id, 'person-1')

  const receipt = await getCalendarIntakeReceiptBySourceIdentity('calendar:stub:acme', 'evt-1', db.tx)
  assert.equal(receipt?.status, 'completed')
  assert.equal(receipt?.interactionId, outcome.interactionId)
  assert.equal(receipt?.providerCursor, 'cursor-2', 'cursor advanced onto the receipt')

  // no person was created and no task was derived
  assert.equal(db.createPersonCalls, 0, 'no person auto-creation from a calendar event')
  assert.equal(db.persons.length, 1, 'only the pre-seeded person exists')
  assert.equal(db.tasks.length, 0, 'no follow-up task noise from an appointment')

  // cursor is readable for the next poll
  assert.equal(await readCalendarIntakeCursor('calendar:stub:acme', db.tx), 'cursor-2')
})

test('lowering: a replayed provider event dedupes to the SAME receipt — no second interaction', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', 'buyer1@example.com')
  const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
  const provider = new StubCalendarProvider([providerEvent()], 'cursor-2')

  const first = await syncCalendarEvents({
    provider, configuration, repositories: db.repositories(), durability, now: () => FIXED_NOW,
  })
  assert.equal(first.outcomes[0].outcome, 'completed')

  // replay: the provider (or a webhook-triggered sync) returns the same event
  const replay = await syncCalendarEvents({
    provider, configuration, repositories: db.repositories(), durability, now: () => FIXED_NOW,
  })
  assert.equal(replay.outcomes[0].outcome, 'completed')
  assert.equal(replay.outcomes[0].created, false, 'replay creates nothing')

  assert.equal(db.interactions.length, 1, 'no duplicate interaction')
  const receipts = db.receipts.filter((r) => r.source_external_id === 'evt-1')
  assert.equal(receipts.length, 1, 'one receipt for the replayed source identity')
  assert.equal(receipts[0].status, 'completed')

  // individual replay through processCalendarEvent also dedupes
  const single = await processCalendarEvent({
    event: providerEvent(),
    configuration,
    repositories: db.repositories(),
    durability,
    cursor: 'cursor-2',
    syncedAt: FIXED_NOW,
  })
  assert.equal(single.outcome, 'completed')
  assert.equal(single.created, false)
  assert.equal(db.interactions.length, 1)
  assert.equal(db.receipts.length, 1)
})

test('lowering: unknown attendee -> resolution_required on the receipt; no person, no interaction', async () => {
  const db = new FakeDb() // NO persons seeded
  const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
  const provider = new StubCalendarProvider([providerEvent()], 'cursor-2')

  const sync = await syncCalendarEvents({
    provider, configuration, repositories: db.repositories(), durability, now: () => FIXED_NOW,
  })
  const outcome = sync.outcomes[0]
  assert.equal(outcome.outcome, 'resolution_required')
  assert.equal(outcome.reason, 'creation_not_allowed')

  assert.equal(db.interactions.length, 0, 'no interaction for an unresolved attendee')
  assert.equal(db.persons.length, 0, 'NO person was auto-created')
  assert.equal(db.createPersonCalls, 0)
  assert.equal(db.tasks.length, 0)

  const receipt = await getCalendarIntakeReceiptBySourceIdentity('calendar:stub:acme', 'evt-1', db.tx)
  assert.equal(receipt?.status, 'resolution_required')
  assert.equal(receipt?.interactionId, undefined)

  // replay stays resolution_required and still creates nothing
  const replay = await processCalendarEvent({
    event: providerEvent(),
    configuration,
    repositories: db.repositories(),
    durability,
    cursor: 'cursor-2',
    syncedAt: FIXED_NOW,
  })
  assert.equal(replay.outcome, 'resolution_required')
  assert.equal(db.interactions.length, 0)
  assert.equal(db.persons.length, 0)
  assert.equal(db.receipts.length, 1)
})

test('lowering: internal-only appointments are recorded rejected, never interactions', async () => {
  const db = new FakeDb()
  const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
  const provider = new StubCalendarProvider(
    [providerEvent({ attendees: [{ kind: 'email', value: 'agent@culebraluxe.example' }] })],
    'cursor-2',
  )

  const sync = await syncCalendarEvents({
    provider, configuration, repositories: db.repositories(), durability, now: () => FIXED_NOW,
  })
  assert.equal(sync.outcomes[0].outcome, 'rejected')
  assert.equal(sync.outcomes[0].reason, 'internal_only')

  assert.equal(db.interactions.length, 0)
  assert.equal(db.tasks.length, 0)
  const receipt = await getCalendarIntakeReceiptBySourceIdentity('calendar:stub:acme', 'evt-1', db.tx)
  assert.equal(receipt?.status, 'rejected')
})

test('lowering: a crash between claim and transition recovers via the stale claim — no duplicate', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', 'buyer1@example.com')
  const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
  const provider = new StubCalendarProvider([providerEvent()], 'cursor-2')

  const first = await syncCalendarEvents({
    provider, configuration, repositories: db.repositories(), durability, now: () => FIXED_NOW,
  })
  assert.equal(first.outcomes[0].outcome, 'completed')
  assert.equal(db.interactions.length, 1)

  // simulate a crash: the receipt is left 'processing' with an ancient claim
  const crashed = db.receipts.find((r) => r.source_external_id === 'evt-1')!
  crashed.status = 'processing'
  crashed.processing_started_at = '2026-08-20T00:00:00.000Z'

  const recovery = await processCalendarEvent({
    event: providerEvent(),
    configuration,
    repositories: db.repositories(),
    durability,
    cursor: 'cursor-2',
    syncedAt: FIXED_NOW,
  })
  assert.equal(recovery.outcome, 'duplicate', 'stale claim re-claimed; interaction deduped')
  assert.equal(db.interactions.length, 1, 'crash recovery never duplicates the interaction')
  assert.equal(db.receipts.length, 1)
})

// ---------------------------------------------------------------------------
// 6. Provider isolation — tokens and raw payloads never leak into CRM rows
// ---------------------------------------------------------------------------

test('isolation: OAuth tokens and raw Google fields never appear in canonical rows', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', 'buyer1@example.com')
  const server = await FakeGoogleCalendarServer.start()
  try {
    server.addEvent({
      id: 'evt-1',
      iCalUID: 'uid-secret@google.com',
      hangoutLink: 'https://meet.google.com/secret-room',
      attendees: [
        { email: 'buyer1@example.com', responseStatus: 'accepted' },
        { email: 'agent@culebraluxe.example', self: true },
      ],
    })
    const { provider, tokenStore } = makeGoogleProvider(server)
    const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))
    const sync = await syncCalendarEvents({
      provider, configuration, repositories: db.repositories(), durability, now: () => FIXED_NOW,
    })
    assert.equal(sync.outcomes[0].outcome, 'completed')
    assert.equal(db.interactions.length, 1)

    const canonicalRows = JSON.stringify({
      interactions: db.interactions,
      receipts: db.receipts,
      persons: db.persons,
    })
    // the OAuth access token never appears in canonical rows
    assert.ok(!canonicalRows.includes('access-1'), 'access token must not leak into canonical rows')
    // raw Google payload fields never cross the seam
    assert.ok(!canonicalRows.includes('uid-secret'), 'iCalUID must not leak')
    assert.ok(!canonicalRows.includes('secret-room'), 'hangoutLink must not leak')
    assert.ok(!canonicalRows.includes('responseStatus'), 'raw attendee state must not leak')
    assert.ok(!canonicalRows.includes('meet.google.com'), 'provider URLs must not leak')

    // the token DOES live in the provider-side token store (in-memory here)
    const cached = await tokenStore.getAccessToken('primary')
    assert.ok(cached, 'provider-side token store holds the access token')
  } finally {
    await server.stop()
  }
})

// ---------------------------------------------------------------------------
// 7. Webhook path — verified notifications trigger a sync
// ---------------------------------------------------------------------------

test('webhook: a verified notification triggers a sync; channel-sync is a no-op', async () => {
  const db = new FakeDb()
  db.seedPerson('person-1', 'buyer1@example.com')
  const server = await FakeGoogleCalendarServer.start()
  try {
    server.addEvent({ id: 'evt-1' })
    const { provider } = makeGoogleProvider(server)
    const durability = createCalendarIntakeDurability(db.tx, (input) => db.createInteraction(input))

    // channel establishment notification carries no data
    const channelSync = await handleCalendarWebhook({
      provider,
      configuration,
      repositories: db.repositories(),
      durability,
      now: () => FIXED_NOW,
      payload: '',
      signature: googHeaders({ 'x-goog-resource-state': 'sync' }),
    })
    assert.equal(channelSync.verification.resourceState, 'sync')
    assert.equal(channelSync.sync, undefined, 'channel-sync notification does not trigger a sync')
    assert.equal(db.interactions.length, 0)

    // an 'updated' notification triggers a sync that lowers the new event
    const updated = await handleCalendarWebhook({
      provider,
      configuration,
      repositories: db.repositories(),
      durability,
      now: () => FIXED_NOW,
      payload: '',
      signature: googHeaders({ 'x-goog-resource-state': 'updated' }),
    })
    assert.equal(updated.verification.resourceState, 'updated')
    assert.ok(updated.sync)
    assert.equal(updated.sync!.outcomes[0].outcome, 'completed')
    assert.equal(db.interactions.length, 1)

    // replayed notification triggers a sync that finds no changes after the
    // cursor advance — the interaction is never duplicated
    const replay = await handleCalendarWebhook({
      provider,
      configuration,
      repositories: db.repositories(),
      durability,
      now: () => FIXED_NOW,
      payload: '',
      signature: googHeaders({ 'x-goog-resource-state': 'updated' }),
    })
    assert.equal(replay.sync!.outcomes.length, 0, 'incremental sync sees no changes')
    assert.equal(db.interactions.length, 1, 'webhook replay never duplicates the interaction')
    assert.equal(db.receipts.length, 1, 'one receipt for the source identity')

    // forged signatures never reach the sync path
    await assert.rejects(
      () =>
        handleCalendarWebhook({
          provider,
          configuration,
          repositories: db.repositories(),
          durability,
          now: () => FIXED_NOW,
          payload: '',
          signature: googHeaders({ 'x-goog-channel-token': 'forged' }),
        }),
      /channel token is invalid/,
    )
    assert.equal(db.interactions.length, 1)
  } finally {
    await server.stop()
  }
})
