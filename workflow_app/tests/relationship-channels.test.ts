import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { summarizeRelationshipEvidence } from '../../lib/relationship-intel/relationship-context'
import { getClientRelationshipChannels } from '../../db/relationship-channels'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// REL-INTEL — Person x Source read model + source-grouped Client History panel.
// The PRIMARY relationship grain is ONE row per canonical Person x communication
// source (mv_client_relationship_channels) — never per-message/per-burst.
// ---------------------------------------------------------------------------

function ev(overrides: Record<string, unknown>) {
  return {
    source: 'apple_messages',
    firstObservedAt: '2025-01-01T00:00:00.000Z',
    inboundCount: 1,
    outboundCount: 1,
    lastObservedAt: '2026-08-01T00:00:00.000Z',
    lastInboundAt: '2026-08-01T00:00:00.000Z',
    lastOutboundAt: '2026-07-01T00:00:00.000Z',
    isTwoWay: true,
    isAutomatedOrBulk: null,
    isOrganizationOrService: null,
    hasEmail: false,
    hasPhone: true,
    coverageNote: null,
    ...overrides,
  }
}

test('D: multiple handles for the same Person+source reduce to ONE source row', () => {
  const summary = summarizeRelationshipEvidence([
    ev({ source: 'apple_messages', inboundCount: 100, outboundCount: 200, firstObservedAt: '2025-01-01T00:00:00.000Z', lastObservedAt: '2026-08-01T00:00:00.000Z' }),
    ev({ source: 'apple_messages', inboundCount: 30, outboundCount: 40, firstObservedAt: '2025-03-01T00:00:00.000Z', lastObservedAt: '2026-08-28T00:00:00.000Z' }),
  ])
  const channels = summary.channels.filter((c) => c.source === 'apple_messages')
  assert.equal(channels.length, 1, 'multiple handles collapse to one source')
  assert.equal(channels[0].inboundCount, 130, 'inbound is SUM')
  assert.equal(channels[0].outboundCount, 240, 'outbound is SUM')
  assert.equal(channels[0].observedCommunicationCount, 370, 'total is inbound+outbound')
  assert.equal(channels[0].lastObservedAt, '2026-08-28T00:00:00.000Z', 'latest wins')
  assert.equal(channels[0].firstObservedAt, '2025-01-01T00:00:00.000Z', 'earliest wins')
})

test('E: source-grain read model returns one normalized row per Person+source', async () => {
  const mvRows = [
    {
      person_id: 'p-ami', source: 'apple_messages', channel: 'imessage',
      first_observed_at: '2025-04-23T13:16:32.439Z', last_contact_at: '2026-08-28T14:52:25.245Z',
      last_inbound_at: '2026-08-28T14:52:25.245Z', last_outbound_at: '2026-08-28T12:05:14.811Z',
      inbound_count: '2434', outbound_count: '2424', total_count: '4858',
      last_direction: 'inbound', two_way: true,
      last_context: 'Did you watch video', last_context_at: '2026-08-28T14:52:25.245Z', last_context_type: 'title',
    },
    {
      person_id: 'p-ami', source: 'gmail_contacts', channel: 'email',
      first_observed_at: null, last_contact_at: '2026-08-15T00:00:00.000Z',
      last_inbound_at: null, last_outbound_at: '2026-08-15T00:00:00.000Z',
      inbound_count: '2', outbound_count: '2', total_count: '4',
      last_direction: 'outbound', two_way: true,
      last_context: null, last_context_at: null, last_context_type: null,
    },
  ]
  const execute = ((_strings: TemplateStringsArray) => Promise.resolve(mvRows)) as unknown as QueryExecutor
  const channels = await getClientRelationshipChannels('p-ami', execute)
  assert.equal(channels.length, 2, 'two sources -> two rows')
  assert.equal(channels[0].source, 'apple_messages')
  assert.equal(channels[0].channel, 'imessage')
  assert.equal(channels[0].totalCount, 4858, 'bigint/string count normalized to JS number')
  assert.equal(channels[0].inboundCount, 2434)
  assert.equal(channels[0].outboundCount, 2424)
  assert.equal(channels[0].lastDirection, 'inbound')
  assert.equal(channels[0].twoWay, true)
  assert.equal(channels[0].lastContext, 'Did you watch video')
  assert.equal(channels[0].lastContactAt, '2026-08-28T14:52:25.245Z')
  assert.equal(channels[1].lastContext, null)
})

test('F: Client directory freshness uses relationship/source state as authoritative', () => {
  const mig = readFileSync('db/migrations/094_mv_client_relationship_channels.sql', 'utf8')
  assert.ok(mig.includes('create materialized view mv_client_relationship_channels'), 'channels MV exists')
  assert.ok(mig.includes('from mv_client_relationship_channels rc'), 'directory last_contact reads source-grain state')
  assert.ok(mig.includes('(select max(i.occurred_at)'), 'detailed interaction is only a fallback')

  const refresh = readFileSync('db/client-read-models.ts', 'utf8')
  const order = [
    refresh.indexOf('refresh materialized view concurrently mv_client_relationship_channels'),
    refresh.indexOf('refresh materialized view concurrently mv_client_directory'),
    refresh.indexOf('refresh materialized view concurrently mv_client_contact_history'),
  ]
  assert.ok(order[0] >= 0 && order[1] > order[0] && order[2] > order[1], 'dependency-safe refresh order')
})

test('G: primary Client History panel renders six compact latest-activity source rows', () => {
  const panel = readFileSync('components/portal/contact-history.tsx', 'utf8')
  assert.ok(panel.includes('CompactRelationshipHeader'), 'aggregate intelligence stays in a compact header')
  assert.ok(panel.includes('SourceActivityRow'), 'source activity uses dense rows rather than oversized nodes')
  assert.ok(panel.includes('No activity connected'), 'missing intake coverage remains visible')
  for (const label of ['Phone', 'iMessage', 'WhatsApp', 'Gmail', 'FaceTime', 'Apple Calendar']) {
    assert.ok(panel.includes(`label: "${label}"`), `${label} has a fixed source row`)
  }
  assert.ok(panel.includes('relationship-channels'), 'primary panel keeps the source-grain route')
  assert.ok(panel.includes('View all'), 'detailed archive remains available')
  assert.ok(panel.includes('&recent=true'), 'bounded history read supplies detail count without becoming primary')

  const repo = readFileSync('db/relationship-channels.ts', 'utf8')
  assert.ok(repo.includes('mv_client_relationship_channels'), 'repo reads the source-grain MV')
})
