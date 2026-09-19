import assert from 'node:assert/strict'
import test from 'node:test'

import { reduceSplit, splitJoinHoldReasons } from '../forge/split-join'

// ---------------------------------------------------------------------------
// FORGE-SPLIT-COMPLETE-PROOF-01 — the two-lane lifecycle RECEIPT, asserted as a shape.
//
// The observation half (two scoped siblings claimed, completing, their outputs contributing, the join
// satisfied, the story reaching QA PASS) comes from a real wave and is recorded in the story's evidence.
// THIS half is the durable one: the receipt's SHAPE, so a later replay cannot fabricate completion and a
// future refactor cannot quietly weaken what "joined" means.
//
// The rule the reducer implements is the one the engine learned the hard way (ENG-FORGE-SPLIT-SIBLING-01,
// 2026-09-18): a fork branch that was never claimed had been advanced to done by the resume door
// (completed_by operator), so the sibling's work item never existed and the join could never be satisfied.
// Missing is failure; silence never satisfies a join; duplicates are idempotent.
// ---------------------------------------------------------------------------

const child = (childId: string, status: 'completed' | 'failed' | 'cancelled', extra: object = {}) => ({
  childId,
  status,
  attempt: 1,
  ...extra,
})

test('split join: two correctly scoped siblings that both complete DO satisfy the join', () => {
  const reduction = reduceSplit({
    expectedIds: ['slot-0', 'slot-1'],
    outcomes: [child('slot-0', 'completed'), child('slot-1', 'completed')],
  })
  assert.equal(reduction.accounted, 2)
  assert.deepEqual(reduction.completed, ['slot-0', 'slot-1'])
  assert.deepEqual(reduction.missing, [])
  assert.equal(reduction.joinSatisfied, true, 'two completing siblings is the observed success case')
})

test('split join: a sibling with NO outcome is MISSING and the join is not satisfied', () => {
  const reduction = reduceSplit({ expectedIds: ['slot-0', 'slot-1'], outcomes: [child('slot-0', 'completed')] })
  assert.deepEqual(reduction.missing, ['slot-1'])
  assert.equal(reduction.joinSatisfied, false, 'silence never satisfies a join')
})

test('split join: a FAILED sibling blocks the join and is named, not forgiven', () => {
  const reduction = reduceSplit({
    expectedIds: ['slot-0', 'slot-1'],
    outcomes: [child('slot-0', 'completed'), child('slot-1', 'failed')],
  })
  assert.deepEqual(reduction.failed, ['slot-1'])
  assert.equal(reduction.joinSatisfied, false)
})

test('split join: duplicate completion is idempotent, and conflicting output is surfaced', () => {
  const idempotent = reduceSplit({
    expectedIds: ['slot-0'],
    outcomes: [child('slot-0', 'completed'), child('slot-0', 'completed')],
  })
  assert.equal(idempotent.joinSatisfied, true, 'a repeated completion does not break the join')

  const conflicting = reduceSplit({
    expectedIds: ['slot-0', 'slot-1'],
    outcomes: [
      child('slot-0', 'completed', { outputKeys: ['media-content-length'] }),
      child('slot-1', 'completed', { outputKeys: ['media-content-length'] }),
    ],
  })
  assert.deepEqual(conflicting.conflicts, ['media-content-length'], 'conflict is named, never silently resolved')
})

test('split join: the hold reasons name what is missing rather than reporting a generic failure', () => {
  const reasons = splitJoinHoldReasons({
    expectedIds: ['slot-0', 'slot-1'],
    outcomes: [child('slot-0', 'failed')],
  })
  assert.ok(reasons.length > 0, 'a failed child must produce a named hold reason')
  assert.match(reasons.join(' | '), /slot-1|slot-0/, 'the reason names the child, so a reader can act')
})
