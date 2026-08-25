import { test } from 'node:test'
import assert from 'node:assert/strict'

import { evaluateRecommendations, type RecommendationInput } from '../../lib/relationship-intel/recommendations'

// ---------------------------------------------------------------------------
// CORE-DAILY-07/08 — deterministic, explainable recommendations (pure).
// ---------------------------------------------------------------------------

const NOW = new Date('2026-01-01T00:00:00.000Z').getTime()

function input(overrides: Partial<RecommendationInput> = {}): RecommendationInput {
  return {
    personId: 'p1',
    followUps: [],
    dismissed: new Set(),
    nowIso: new Date(NOW).toISOString(),
    dueSoonWindowMs: 3 * 86400000,
    quietAfterMs: 45 * 86400000,
    ...overrides,
  }
}

test('CORE-DAILY-07: overdue commitment surfaces with reason + evidence pointer', () => {
  const recs = evaluateRecommendations(input({
    followUps: [{ id: 't1', dueAt: new Date(NOW - 86400000).toISOString(), title: 'Call back' }],
  }))
  assert.equal(recs.length, 1)
  assert.equal(recs[0].code, 'overdue_relationship_commitment')
  assert.equal(recs[0].explanationCode, 'overdue_due_at_lt_now')
  assert.deepEqual(recs[0].evidencePointers, ['task:t1'])
  assert.ok(recs[0].reason.includes('overdue'))
})

test('CORE-DAILY-07: due-soon commitment appears', () => {
  const recs = evaluateRecommendations(input({
    followUps: [{ id: 't2', dueAt: new Date(NOW + 86400000).toISOString(), title: 'Send info' }],
  }))
  assert.equal(recs[0].code, 'due_soon_relationship_commitment')
})

test('CORE-DAILY-07: unanswered inbound (meaningful) surfaces when no open follow-up', () => {
  const recs = evaluateRecommendations(input({
    followUps: [],
    evidence: { lastMeaningfulContactAt: null, lastInboundAt: new Date(NOW - 86400000).toISOString(), lastOutboundAt: new Date(NOW - 7 * 86400000).toISOString(), twoWay: false, hasEvidence: true, coverageLimited: false },
  }))
  assert.equal(recs[0].code, 'unanswered_inbound')
  assert.equal(recs[0].explanationCode, 'inbound_after_last_outbound')
})

test('CORE-DAILY-07: two-way without a next step surfaces', () => {
  const recs = evaluateRecommendations(input({
    followUps: [],
    evidence: { lastMeaningfulContactAt: null, lastInboundAt: null, lastOutboundAt: null, twoWay: true, hasEvidence: true, coverageLimited: false },
  }))
  assert.equal(recs[0].code, 'two_way_without_next_step')
})

test('CORE-DAILY-07: quiet past client surfaces', () => {
  const recs = evaluateRecommendations(input({
    followUps: [],
    evidence: { lastMeaningfulContactAt: new Date(NOW - 60 * 86400000).toISOString(), lastInboundAt: null, lastOutboundAt: null, twoWay: false, hasEvidence: true, coverageLimited: false },
  }))
  assert.equal(recs[0].code, 'quiet_past_client')
})

test('CORE-DAILY-08: dismissed key is suppressed on regeneration', () => {
  const base = input({
    followUps: [{ id: 't3', dueAt: new Date(NOW - 86400000).toISOString(), title: 'Call back' }],
  })
  const before = evaluateRecommendations(base)
  assert.equal(before.length, 1)
  const dismissed = evaluateRecommendations({
    ...base,
    dismissed: new Set([`p1:overdue_relationship_commitment`]),
  })
  assert.equal(dismissed.length, 0)
})

test('CORE-DAILY-08: regeneration is deterministic (no duplicate multiplication)', () => {
  const base = input({
    followUps: [{ id: 't4', dueAt: new Date(NOW - 86400000).toISOString(), title: 'Call back' }],
  })
  const a = evaluateRecommendations(base)
  const b = evaluateRecommendations(base)
  assert.equal(a.length, 1)
  assert.equal(b.length, 1)
  assert.deepEqual(a.map((r) => r.code), b.map((r) => r.code))
})

test('CORE-DAILY-08: bulk-only evidence never produces a meaningful recommendation', () => {
  // evidence is the meaningful summary; if hasEvidence is false (bulk only), no recommendation.
  const recs = evaluateRecommendations(input({ followUps: [], evidence: undefined }))
  assert.deepEqual(recs, [])
})
