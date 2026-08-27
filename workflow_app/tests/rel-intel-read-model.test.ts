import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  summarizeRelationshipEvidence,
  type RelationshipEvidenceForContext,
} from '../../lib/relationship-intel/relationship-context'

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
  // Last observed is dominated by the (ignored) bulk row; meaningful is the real one.
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
