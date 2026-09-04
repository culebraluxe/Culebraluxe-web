import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  summarizeRelationshipEvidence,
  type RelationshipEvidenceForContext,
} from '../../lib/relationship-intel/relationship-context'
import { getRelationshipEvidenceForPersons } from '../../db/relationship-evidence'
import type { QueryExecutor } from '../../db/query-executor'
import { buildAggregateEvidenceItems } from '../../db/contact-history'

// ---------------------------------------------------------------------------
// REL-INTEL — relationship read-model distinctions (pure).
// last-observed vs meaningful contact; bulk/service never refresh freshness;
// Apple-only / Gmail-only / both; empty and limited states. No database.
// ---------------------------------------------------------------------------

function ev(overrides: Partial<RelationshipEvidenceForContext> = {}): RelationshipEvidenceForContext {
  return {
    source: 'gmail_contacts',
    inboundCount: 3,
    outboundCount: 2,
    lastObservedAt: '2013-12-31T00:00:00.000Z',
    isAutomatedOrBulk: false,
    isOrganizationOrService: false,
    hasEmail: true,
    hasPhone: false,
    ...overrides,
  }
}

test('REL-INTEL: bulk-only evidence never produces meaningful contact', () => {
  const s = summarizeRelationshipEvidence([
    ev({ source: 'gmail_contacts', lastObservedAt: '2013-12-31T00:00:00.000Z', isAutomatedOrBulk: true }),
  ])
  assert.equal(s.hasEvidence, true)
  assert.equal(s.lastObservedAt, '2013-12-31T00:00:00.000Z')
  assert.equal(s.lastMeaningfulContactAt, null)
  assert.equal(s.reason?.includes('Bulk'), true)
})

test('REL-INTEL: service/organization-only evidence has no meaningful contact', () => {
  const s = summarizeRelationshipEvidence([
    ev({ isOrganizationOrService: true }),
  ])
  assert.equal(s.lastMeaningfulContactAt, null)
  assert.equal(s.reason?.includes('Service'), true)
})

test('REL-INTEL: last observed differs from last meaningful when bulk is present', () => {
  const s = summarizeRelationshipEvidence([
    ev({ source: 'gmail_contacts', lastObservedAt: '2013-12-31T00:00:00.000Z', isAutomatedOrBulk: true }),
    ev({ source: 'apple_contacts', lastObservedAt: '2012-06-01T00:00:00.000Z', isAutomatedOrBulk: false }),
  ])
  assert.equal(s.lastObservedAt, '2013-12-31T00:00:00.000Z')
  assert.equal(s.lastMeaningfulContactAt, '2012-06-01T00:00:00.000Z')
})

test('REL-INTEL: meaningful contact prefers outbound/inbound when no observed present', () => {
  const s = summarizeRelationshipEvidence([
    ev({ lastObservedAt: null, lastOutboundAt: '2013-11-01T00:00:00.000Z' }),
  ])
  assert.equal(s.lastMeaningfulContactAt, '2013-11-01T00:00:00.000Z')
})

test('REL-INTEL: empty evidence is a clean empty/limited state', () => {
  const s = summarizeRelationshipEvidence([])
  assert.equal(s.hasEvidence, false)
  assert.equal(s.sources.length, 0)
  assert.equal(s.lastMeaningfulContactAt, null)
  assert.equal(s.reason, null)
})

test('REL-INTEL: sources aggregate across Apple + Gmail', () => {
  const s = summarizeRelationshipEvidence([
    ev({ source: 'apple_contacts' }),
    ev({ source: 'gmail_contacts' }),
  ])
  assert.deepEqual([...s.sources].sort(), ['apple_contacts', 'gmail_contacts'])
  assert.equal(s.twoWay, false)
})

test('REL-INTEL: coverage limitation is surfaced', () => {
  const s = summarizeRelationshipEvidence([
    ev({ coverageNote: 'partial sweep omits early history' }),
  ])
  assert.equal(s.coverageLimited, true)
})

test('REL-INTEL: two-way indicator is aggregated', () => {
  const s = summarizeRelationshipEvidence([
    ev({ isTwoWay: true }),
  ])
  assert.equal(s.twoWay, true)
})

test('REL-INTEL: aggregate communication counts stay distinct from canonical interactions', () => {
  const s = summarizeRelationshipEvidence([
    ev({ source: 'apple_messages', inboundCount: 2431, outboundCount: 2413, isTwoWay: true, lastObservedAt: null }),
  ])
  assert.equal(s.inboundCount, 2431)
  assert.equal(s.outboundCount, 2413)
  assert.equal(s.observedCommunicationCount, 4844)
  assert.equal(s.lastObservedAt, null)
})

function evidenceRow(overrides: Record<string, unknown> = {}) {
  const D = new Date('2026-02-25T17:36:43.709Z')
  return {
    id: 'e-1',
    source: 'apple_messages',
    source_account: 'acct',
    source_identity_key: '+1',
    source_label: null,
    display_name: 'Alicia Geigel',
    organization: null,
    emails: [],
    phones: [],
    first_observed_at: D,
    last_observed_at: D,
    last_inbound_at: null,
    last_outbound_at: D,
    inbound_count: 0,
    outbound_count: 1,
    is_two_way: false,
    is_owner_initiated: false,
    is_automated_or_bulk: false,
    is_organization_or_service: false,
    known_apple_contact: true,
    has_email: false,
    has_phone: true,
    coverage_note: null,
    canonical_person_id: 'p-alicia',
    match_method: 'exact',
    match_confidence: 'high',
    review_state: 'exact_linked',
    match_reason: null,
    rule_version: 'v1',
    evidence_fingerprint: 'fp',
    updated_at: D,
    ...overrides,
  }
}

test('REL-INTEL: repository normalizes Date timestamptz into ISO strings (no Date leaks)', async () => {
  const execute: QueryExecutor = (async () => [
    evidenceRow(),
    evidenceRow({
      id: 'e-2',
      source: 'gmail_contacts',
      first_observed_at: new Date('2013-10-25T23:05:13.000Z'),
      last_observed_at: new Date('2013-12-26T16:41:49.000Z'),
      last_inbound_at: new Date('2013-12-20T00:00:00.000Z'),
      last_outbound_at: new Date('2013-12-26T16:41:49.000Z'),
    }),
  ]) as QueryExecutor

  const byPerson = await getRelationshipEvidenceForPersons(['p-alicia'], execute)
  const rows = byPerson['p-alicia'] ?? []
  assert.equal(rows.length, 2)

  for (const r of rows) {
    for (const field of ['firstObservedAt', 'lastObservedAt', 'lastInboundAt', 'lastOutboundAt'] as const) {
      const v = r[field]
      assert.ok(v === null || typeof v === 'string', `${field} must be string|null, got ${typeof v}`)
    }
    assert.equal(typeof r.updatedAt, 'string')
  }
  assert.equal(rows[0].lastObservedAt, '2026-02-25T17:36:43.709Z')
  assert.equal(rows[1].lastObservedAt, '2013-12-26T16:41:49.000Z')

  const sum = summarizeRelationshipEvidence(rows)
  assert.equal(sum.hasEvidence, true)
  assert.ok(sum.sources.length >= 2)
  assert.equal(typeof sum.lastObservedAt, 'string')
  assert.ok(sum.channels.length >= 1)
})

function channelsFor(evidence: RelationshipEvidenceForContext[]) {
  return summarizeRelationshipEvidence(evidence).channels
}

test('REL-INTEL: gmail evidence-only produces ONE aggregate Email item (no fabrication)', () => {
  const items = buildAggregateEvidenceItems(
    channelsFor([
      ev({ source: 'gmail_contacts', inboundCount: 9, outboundCount: 1, isTwoWay: true, firstObservedAt: '2013-10-25T00:00:00.000Z', lastObservedAt: '2013-12-26T00:00:00.000Z' }),
    ]),
    new Set(),
  )
  assert.equal(items.length, 1)
  const item = items[0]
  assert.equal(item.kind, 'aggregate_evidence')
  assert.equal(item.channel, 'email')
  assert.equal(item.source, 'email')
  assert.equal(item.totalCount, 10)
  assert.equal(item.inboundCount, 9)
  assert.equal(item.outboundCount, 1)
  assert.equal(item.isTwoWay, true)
  assert.equal(item.firstObservedAt, '2013-10-25T00:00:00.000Z')
  assert.equal(item.lastObservedAt, '2013-12-26T00:00:00.000Z')
})

test('REL-INTEL: identity-only Apple Contacts never yields a communication item', () => {
  const items = buildAggregateEvidenceItems(
    channelsFor([ev({ source: 'apple_contacts', inboundCount: 0, outboundCount: 0 })]),
    new Set(),
  )
  assert.deepEqual(items, [])
})

test('REL-INTEL: a source already represented by detailed events is NOT duplicated', () => {
  const items = buildAggregateEvidenceItems(
    channelsFor([ev({ source: 'apple_messages', inboundCount: 1, outboundCount: 1 })]),
    new Set(['apple_messages']),
  )
  assert.deepEqual(items, [])
})

test('REL-INTEL: multi-source, only the uncovered presentation source gets an aggregate item', () => {
  const items = buildAggregateEvidenceItems(
    channelsFor([
      ev({ source: 'apple_messages', inboundCount: 2431, outboundCount: 2413 }),
      ev({ source: 'gmail_contacts', inboundCount: 9, outboundCount: 1 }),
    ]),
    new Set(['apple_messages']),
  )
  assert.equal(items.length, 1)
  assert.equal(items[0].source, 'email')
})

test('REL-INTEL: zero channels produces no aggregate items', () => {
  assert.deepEqual(buildAggregateEvidenceItems(channelsFor([]), new Set()), [])
})
