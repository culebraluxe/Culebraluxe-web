import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { summarizeRelationshipEvidence } from '../../lib/relationship-intel/relationship-context'
import {
  getClientContactHistory,
  RECENT_TIMELINE_LIMIT,
} from '../../db/contact-history'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// CLIENTS — source-neutral relationship timeline / Cloze-style summary proofs.
// ---------------------------------------------------------------------------

// --- Relationship summary (pure) -------------------------------------------

function ev(overrides: Record<string, unknown>) {
  return {
    source: 'apple_messages',
    firstObservedAt: '2023-01-01T00:00:00.000Z',
    inboundCount: 3,
    outboundCount: 1,
    lastObservedAt: '2023-06-01T00:00:00.000Z',
    lastInboundAt: '2023-06-01T00:00:00.000Z',
    lastOutboundAt: '2023-05-01T00:00:00.000Z',
    isTwoWay: true,
    isAutomatedOrBulk: null,
    isOrganizationOrService: null,
    hasEmail: false,
    hasPhone: true,
    coverageNote: null,
    ...overrides,
  }
}

test('timeline 1: newest communication across channels wins last_contact_at', () => {
  const summary = summarizeRelationshipEvidence([
    ev({ source: 'apple_messages', lastObservedAt: '2023-06-01T00:00:00.000Z', lastInboundAt: '2023-06-01T00:00:00.000Z', lastOutboundAt: '2023-05-01T00:00:00.000Z', isTwoWay: true }),
    ev({ source: 'gmail_contacts', lastObservedAt: '2023-08-15T00:00:00.000Z', lastInboundAt: null, lastOutboundAt: '2023-08-15T00:00:00.000Z', hasEmail: true, hasPhone: false }),
  ])
  assert.equal(summary.lastObservedAt, '2023-08-15T00:00:00.000Z', 'email (Aug) is newer than iMessage (Jun)')
})

test('timeline 2: winning channel is the newest source projection', () => {
  const summary = summarizeRelationshipEvidence([
    ev({ source: 'apple_messages', lastObservedAt: '2023-06-01T00:00:00.000Z' }),
    ev({ source: 'gmail_contacts', lastObservedAt: '2023-08-15T00:00:00.000Z', hasEmail: true, hasPhone: false }),
  ])
  assert.equal(summary.channels[0].source, 'gmail_contacts', 'channels sorted newest-first')
  assert.equal(summary.channels[0].channel, 'email')
})

test('timeline 3: winning direction is returned from the newest projection', () => {
  const summary = summarizeRelationshipEvidence([
    ev({ source: 'gmail_contacts', lastObservedAt: '2023-08-15T00:00:00.000Z', lastInboundAt: null, lastOutboundAt: '2023-08-15T00:00:00.000Z', hasEmail: true, hasPhone: false }),
  ])
  assert.equal(summary.channels[0].lastOutboundAt, summary.channels[0].lastObservedAt)
  assert.equal(summary.channels[0].lastInboundAt, null)
})

test('timeline 6/7/8/9/10: first/last-inbound/last-outbound/total/two-way are correct', () => {
  const summary = summarizeRelationshipEvidence([
    ev({ source: 'apple_messages', firstObservedAt: '2023-01-01T00:00:00.000Z', inboundCount: 3, outboundCount: 1, lastInboundAt: '2023-06-01T00:00:00.000Z', lastOutboundAt: '2023-05-01T00:00:00.000Z', lastObservedAt: '2023-06-01T00:00:00.000Z' }),
    ev({ source: 'gmail_contacts', firstObservedAt: '2023-03-01T00:00:00.000Z', inboundCount: 2, outboundCount: 2, lastInboundAt: '2023-08-10T00:00:00.000Z', lastOutboundAt: '2023-08-15T00:00:00.000Z', lastObservedAt: '2023-08-15T00:00:00.000Z', hasEmail: true, hasPhone: false }),
  ])
  assert.equal(summary.firstObservedAt, '2023-01-01T00:00:00.000Z')
  assert.equal(summary.lastInboundAt, '2023-08-10T00:00:00.000Z')
  assert.equal(summary.lastOutboundAt, '2023-08-15T00:00:00.000Z')
  assert.equal(summary.observedCommunicationCount, 8)
  assert.equal(summary.twoWay, true)
})

test('timeline 11: per-channel last activity is correct', () => {
  const summary = summarizeRelationshipEvidence([
    ev({ source: 'apple_messages', lastObservedAt: '2023-06-01T00:00:00.000Z', inboundCount: 3, outboundCount: 1 }),
    ev({ source: 'gmail_contacts', lastObservedAt: '2023-08-15T00:00:00.000Z', inboundCount: 2, outboundCount: 2, hasEmail: true, hasPhone: false }),
  ])
  const imessage = summary.channels.find((c) => c.channel === 'imessage')
  const email = summary.channels.find((c) => c.channel === 'email')
  assert.equal(imessage?.lastObservedAt, '2023-06-01T00:00:00.000Z')
  assert.equal(email?.lastObservedAt, '2023-08-15T00:00:00.000Z')
  assert.equal(imessage?.observedCommunicationCount, 4)
})

// --- Bounded recent timeline (read model) ----------------------------------

type HistoryRow = {
  interaction_id: string
  channel: string
  direction: string | null
  occurred_at: string
  title: string | null
  summary: string | null
}

function makeExecutor(mvRows: HistoryRow[]): QueryExecutor {
  const tx = ((strings: TemplateStringsArray): Promise<unknown[]> => {
    const t = strings.join('?').replace(/\s+/g, ' ')
    if (t.includes('select count(*)') && t.includes('mv_client_contact_history')) {
      return Promise.resolve([{ total: mvRows.length }])
    }
    if (t.includes('mv_client_contact_history mv')) {
      return Promise.resolve(mvRows as unknown[])
    }
    if (t.includes('integration_relationship_evidence')) {
      return Promise.resolve([])
    }
    if (t.includes('from interaction')) {
      return Promise.resolve([])
    }
    return Promise.resolve([])
  }) as unknown as QueryExecutor
  return tx
}

function historyRow(id: string, iso: string): HistoryRow {
  return { interaction_id: id, channel: 'imessage', direction: 'outbound', occurred_at: iso, title: 'iMessage', summary: null }
}

test('timeline 12/13: recent timeline is bounded to 10 and newest-first', async () => {
  const rows: HistoryRow[] = Array.from({ length: 25 }, (_, i) =>
    historyRow(`m${i}`, `2024-01-${String(30 - i).padStart(2, '0')}T12:00:00.000Z`),
  )
  const execute = makeExecutor(rows)
  const recent = await getClientContactHistory('p1', { recent: true }, execute)
  assert.equal(recent.recent, true)
  assert.ok(recent.rows.length <= RECENT_TIMELINE_LIMIT, `recent bounded (got ${recent.rows.length})`)
  const dates = recent.rows.map((r) => (r.kind === 'detail' ? r.startedAt ?? '' : r.lastObservedAt ?? ''))
  for (let i = 1; i < dates.length; i++) {
    assert.ok(dates[i - 1] >= dates[i], `row ${i - 1} must be >= row ${i}`)
  }
})

test('timeline 14/15: primary UI is source-grain (one node per source); full archive is secondary "View all"', () => {
  const src = readFileSync('components/portal/contact-history.tsx', 'utf8')
  assert.ok(src.includes('relationship-channels'), 'default load reads the source-grain channels route')
  assert.ok(src.includes('SourceChannelNode'), 'one gold node per communication source')
  assert.ok(src.includes('View all'), 'View all reveals the full detailed archive')
})

test('timeline 16: Client read model is source-neutral (no raw handle/staging table reads)', () => {
  const src = readFileSync('db/contact-history.ts', 'utf8')
  for (const t of ['from l_person', 'from integration_staged_contact_profile', 'from chat', 'from handle']) {
    assert.ok(!src.includes(t), `db/contact-history must not read ${t}`)
  }
})

test('timeline 17: normal PROD Apple Messages sync does NOT run a second full replay', () => {
  const src = readFileSync('scripts/apple-messages-intake.ts', 'utf8')
  assert.ok(!src.includes('=== REPLAY (second run'), 'no second full replay in the CLI')
  assert.ok(!src.includes('const second = await runAppleMessagesIntake'), 'single intake pass only')
  assert.ok(src.includes('runAppleMessagesIntake(target, dir, execute)'), 'one intake call')
})

test('timeline 18: idempotency remains proven in the regression harness', () => {
  const src = readFileSync('workflow_app/tests/apple-messages-intake.test.ts', 'utf8')
  assert.ok(src.includes('replay fingerprint is deterministic (idempotent source identity)'))
})

test('timeline 19/20: New/Add Client button removed, Edit preserved', () => {
  const manager = readFileSync('components/portal/client-manager.tsx', 'utf8')
  const editor = readFileSync('components/portal/client-editor.tsx', 'utf8')
  assert.ok(!manager.includes('setShowCreate(true)'), 'no create entry point in the manager')
  assert.ok(manager.includes('showEdit'), 'edit path preserved')
  assert.ok(editor.includes('mode === "create"') && editor.includes('createClientAction'), 'create backend preserved')
})

