import test from 'node:test'
import assert from 'node:assert/strict'

import { loadGoogleCalendarConfig } from '../../lib/calendar/google/config'
import { GoogleCalendarClient } from '../../lib/calendar/google/client'
import {
  GoogleCalendarProvider,
  lowerGoogleCalendarEvent,
} from '../../lib/calendar/google/adapter'
import { verifyGoogleCalendarWebhook } from '../../lib/calendar/google/webhook'
import { InMemoryCalendarTokenStore } from '../../lib/calendar/token-store'

const FIXED_NOW = new Date('2026-08-22T00:00:00.000Z')

function config(overrides: Record<string, string> = {}) {
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
    GOOGLE_CALENDAR_TOKEN_ENDPOINT: 'https://oauth.test/token',
    GOOGLE_CALENDAR_API_BASE: 'https://calendar.test/calendar/v3',
    ...overrides,
  })
}

function rawEvent(id: string, updated: string) {
  return {
    id,
    status: 'confirmed',
    summary: 'Property viewing',
    description: 'Buyer viewing',
    start: { dateTime: updated },
    updated,
    organizer: { email: 'agent@culebraluxe.example', self: true },
    attendees: [
      { email: 'buyer1@example.com', responseStatus: 'accepted' },
      { email: 'agent@culebraluxe.example', self: true },
    ],
    iCalUID: `${id}@google.com`,
    hangoutLink: `https://meet.google.com/${id}`,
  }
}

function headers(overrides: Record<string, string> = {}) {
  return {
    'x-goog-channel-id': 'chan-1',
    'x-goog-resource-id': 'res-1',
    'x-goog-resource-state': 'updated',
    'x-goog-message-number': '3',
    'x-goog-channel-token': 'webhook-token',
    ...overrides,
  }
}

test('config reads required keys, defaults safely, and never echoes secrets', () => {
  const cfg = config()
  assert.equal(cfg.clientId, 'client-id')
  assert.equal(cfg.calendarId, 'primary')
  assert.equal(cfg.accountNamespace, 'primary')
  assert.equal(cfg.syncMode, 'updated_time')
  assert.equal(cfg.lookbackDays, 14)
  assert.equal(cfg.webhookChannelToken, 'webhook-token')
  assert.throws(
    () => loadGoogleCalendarConfig({ GOOGLE_CLIENT_ID: 'c', GOOGLE_CALENDAR_ID: 'primary' }),
    /GOOGLE_CLIENT_SECRET, GOOGLE_REFRESH_TOKEN/,
  )
  try {
    loadGoogleCalendarConfig({ GOOGLE_CLIENT_ID: 'super-secret-value' })
    assert.fail('expected config failure')
  } catch (error) {
    assert.ok(!String(error).includes('super-secret-value'))
  }
})

test('Google event lowering is source-neutral and drops cancelled/raw provider fields', () => {
  const cancelled = lowerGoogleCalendarEvent(
    { id: 'evt-x', status: 'cancelled', start: { dateTime: '2026-08-20T14:00:00Z' }, updated: '2026-08-20T13:00:00Z' },
    'primary',
  )
  assert.equal(cancelled, null)

  const lowered = lowerGoogleCalendarEvent(rawEvent('evt-1', '2026-08-20T14:00:00.000Z'), 'primary')
  assert.ok(lowered)
  assert.equal(lowered!.provider, 'google')
  assert.equal(lowered!.accountNamespace, 'primary')
  assert.equal(lowered!.providerEventId, 'evt-1')
  assert.equal(lowered!.organizer, 'owned')
  assert.deepEqual(lowered!.attendees, [
    { kind: 'email', value: 'buyer1@example.com' },
    { kind: 'email', value: 'agent@culebraluxe.example' },
  ])
  const serialized = JSON.stringify(lowered)
  assert.ok(!serialized.includes('iCalUID'))
  assert.ok(!serialized.includes('hangoutLink'))
  assert.ok(!serialized.includes('responseStatus'))
})

test('updated-time first sync uses the injected clock, never the wall clock', async () => {
  const calls: Array<Record<string, unknown>> = []
  const fakeClient = {
    async listEvents(params: Record<string, unknown>) {
      calls.push(params)
      return {
        items: [rawEvent('evt-1', '2026-08-20T10:00:00.000Z')],
        nextSyncToken: 'sync-1',
      }
    },
    async getEvent() { return null },
  } as unknown as GoogleCalendarClient

  const provider = new GoogleCalendarProvider(
    config(),
    fakeClient,
    { now: () => FIXED_NOW },
  )
  const result = await provider.listEventsSince(null)
  assert.equal(result.events.length, 1)
  assert.equal(result.events[0].providerEventId, 'evt-1')
  assert.equal(result.nextCursor, '2026-08-20T10:00:00.001Z')
  assert.equal(
    calls[0].timeMin,
    '2026-08-08T00:00:00.000Z',
    '14-day lookback is anchored to the injected test clock',
  )
  assert.equal(calls[0].orderBy, 'updated')
})

test('updated-time incremental sync uses cursor and preserves it when no updates exist', async () => {
  const calls: Array<Record<string, unknown>> = []
  const fakeClient = {
    async listEvents(params: Record<string, unknown>) {
      calls.push(params)
      return { items: [], nextSyncToken: null }
    },
    async getEvent() { return null },
  } as unknown as GoogleCalendarClient
  const provider = new GoogleCalendarProvider(config(), fakeClient, { now: () => FIXED_NOW })
  const cursor = '2026-08-20T10:05:00.001Z'
  const result = await provider.listEventsSince(cursor)
  assert.deepEqual(result.events, [])
  assert.equal(result.nextCursor, cursor)
  assert.equal(calls[0].updatedMin, cursor)
  assert.equal('timeMin' in calls[0], false)
})

test('sync-token mode uses bounded first sync then the opaque provider token', async () => {
  const calls: Array<Record<string, unknown>> = []
  let n = 0
  const fakeClient = {
    async listEvents(params: Record<string, unknown>) {
      calls.push(params)
      n += 1
      return { items: [], nextSyncToken: `sync-${n}` }
    },
    async getEvent() { return null },
  } as unknown as GoogleCalendarClient
  const provider = new GoogleCalendarProvider(
    config({ GOOGLE_CALENDAR_SYNC_MODE: 'sync_token' }),
    fakeClient,
    { now: () => FIXED_NOW },
  )
  const first = await provider.listEventsSince(null)
  assert.equal(first.nextCursor, 'sync-1')
  assert.equal(calls[0].timeMin, '2026-08-08T00:00:00.000Z')
  const second = await provider.listEventsSince(first.nextCursor)
  assert.equal(second.nextCursor, 'sync-2')
  assert.equal(calls[1].syncToken, 'sync-1')
})

test('OAuth refresh is cached against the same deterministic clock', async () => {
  let tokenRequests = 0
  const tokenStore = new InMemoryCalendarTokenStore(() => FIXED_NOW)
  const cfg = config()
  const client = new GoogleCalendarClient(cfg, {
    tokenStore,
    now: () => FIXED_NOW,
    sleep: async () => {},
    fetchFn: async (url) => {
      if (String(url) === cfg.tokenEndpoint) {
        tokenRequests += 1
        return new Response(JSON.stringify({ access_token: 'access-1', expires_in: 3600 }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      throw new Error(`unexpected fetch: ${String(url)}`)
    },
  })
  assert.equal(await client.accessToken(), 'access-1')
  assert.equal(await client.accessToken(), 'access-1')
  assert.equal(tokenRequests, 1, 'second access reuses provider-side token cache')
})

test('OAuth cache refreshes when the fixed clock advances past expiry margin', async () => {
  let now = new Date('2026-08-22T00:00:00.000Z')
  let tokenRequests = 0
  const tokenStore = new InMemoryCalendarTokenStore(() => now)
  const cfg = config()
  const client = new GoogleCalendarClient(cfg, {
    tokenStore,
    now: () => now,
    sleep: async () => {},
    fetchFn: async () => {
      tokenRequests += 1
      return new Response(JSON.stringify({ access_token: `access-${tokenRequests}`, expires_in: 120 }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      })
    },
  })
  assert.equal(await client.accessToken(), 'access-1')
  now = new Date('2026-08-22T00:01:10.000Z')
  assert.equal(await client.accessToken(), 'access-2')
  assert.equal(tokenRequests, 2)
})

test('webhook verification accepts known Google push state and fails closed', () => {
  assert.equal(
    verifyGoogleCalendarWebhook('', headers(), 'webhook-token').resourceState,
    'updated',
  )
  assert.equal(
    verifyGoogleCalendarWebhook('', headers({ 'x-goog-resource-state': 'sync' }), 'webhook-token').resourceState,
    'sync',
  )
  assert.throws(
    () => verifyGoogleCalendarWebhook('', headers({ 'x-goog-channel-token': 'forged' }), 'webhook-token'),
    /channel token is invalid/,
  )
  assert.throws(
    () => verifyGoogleCalendarWebhook('', headers(), null),
    /not configured/,
  )
  assert.throws(
    () => verifyGoogleCalendarWebhook('', headers({ 'x-goog-resource-state': 'bogus' }), 'webhook-token'),
    /invalid X-Goog-Resource-State/,
  )
})

test('provider getEvent lowers the current event and preserves null for unknown ids', async () => {
  const fakeClient = {
    async listEvents() { return { items: [], nextSyncToken: null } },
    async getEvent(id: string) {
      return id === 'evt-1' ? rawEvent('evt-1', '2026-08-20T10:00:00.000Z') : null
    },
  } as unknown as GoogleCalendarClient
  const provider = new GoogleCalendarProvider(config(), fakeClient, { now: () => FIXED_NOW })
  assert.equal((await provider.getEvent('evt-1'))?.providerEventId, 'evt-1')
  assert.equal(await provider.getEvent('missing'), null)
})
