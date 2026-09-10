import assert from 'node:assert/strict'
import test from 'node:test'

import { reduceSplit, splitJoinHoldReasons, type SplitOutcome } from '../forge/split-join'

// ENG-FORGE-SPLIT-01 — the pre-`lead_post` join gate.
//
// These are the guarantees that make the SPLIT lane safe to turn on: lead_post may
// integrate ONLY when every expected child finished, none failed or was cancelled,
// no sibling claims the same output, and each child recorded its OWN candidate SHA.
// A missing child is failure — silence never satisfies the join.

const ok = (childId: string, attempt = 1, sha = `sha-${childId}`): SplitOutcome => ({
  childId,
  status: 'completed',
  attempt,
  candidateSha: sha,
})

test('a fully accounted fan-out is the only thing that satisfies the join', () => {
  assert.deepEqual(splitJoinHoldReasons({ expectedIds: ['a', 'b'], outcomes: [ok('a'), ok('b')] }), [])
})

test('a child that never finished is a HOLD, not an absence of evidence', () => {
  const reasons = splitJoinHoldReasons({ expectedIds: ['a', 'b'], outcomes: [ok('a')] })
  assert.equal(reasons.length, 1)
  assert.match(reasons[0], /never reached a terminal state: b/)
})

test('failed and cancelled children each block the join by name', () => {
  const failed = splitJoinHoldReasons({
    expectedIds: ['a', 'b'],
    outcomes: [ok('a'), { childId: 'b', status: 'failed', attempt: 2 }],
  })
  assert.match(failed.join('\n'), /split children failed: b/)
  const cancelled = splitJoinHoldReasons({
    expectedIds: ['a', 'b'],
    outcomes: [ok('a'), { childId: 'b', status: 'cancelled', attempt: 1 }],
  })
  assert.match(cancelled.join('\n'), /cancelled\/paused: b/)
})

test('siblings claiming the same output key block the join', () => {
  const reasons = splitJoinHoldReasons({
    expectedIds: ['a', 'b'],
    outcomes: [
      { ...ok('a'), outputKeys: ['workflow_app/forge/x.ts'] },
      { ...ok('b'), outputKeys: ['workflow_app/forge/x.ts'] },
    ],
  })
  assert.match(reasons.join('\n'), /same output/)
})

test('a child that finished with no SHA of its own is never integrable', () => {
  const reasons = splitJoinHoldReasons({
    expectedIds: ['a', 'b'],
    outcomes: [ok('a'), ok('b')],
    unrecordedCandidates: ['b'],
  })
  assert.match(reasons.join('\n'), /no candidate SHA recorded from their own workspace: b/)
})

test('a retried child is counted once, and the later attempt wins', () => {
  const reduction = reduceSplit({
    expectedIds: ['a', 'b'],
    outcomes: [
      { childId: 'a', status: 'failed', attempt: 1 },
      { childId: 'a', status: 'completed', attempt: 2, candidateSha: 'sha-a2' },
      ok('b'),
    ],
  })
  assert.deepEqual(reduction.duplicates, ['a'])
  assert.deepEqual(reduction.failed, [])
  assert.equal(reduction.joinSatisfied, true)
  assert.deepEqual(
    splitJoinHoldReasons({
      expectedIds: ['a', 'b'],
      outcomes: [
        { childId: 'a', status: 'failed', attempt: 1 },
        { childId: 'a', status: 'completed', attempt: 2, candidateSha: 'sha-a2' },
        ok('b'),
      ],
    }),
    [],
  )
})

// ENG-FORGE-SPLIT-DOGFOOD-01 — duplicated TERMINAL child outcomes.
//
// `duplicates` records every repeat outcome (a failed attempt then a retry is a
// duplicate, and that is fine). But two completed terminal outcomes for the same
// child mean the reducer silently collapsed a duplicate terminal result. The
// reduction must name it, and the pre-lead_post gate must HOLD on it.

test('a single completed outcome per child yields no duplicatedTerminal', () => {
  const reduction = reduceSplit({
    expectedIds: ['a', 'b'],
    outcomes: [ok('a'), ok('b')],
  })
  assert.deepEqual(reduction.duplicatedTerminal, [])
})

test('a failed-then-retried child is a duplicate, not a duplicated terminal', () => {
  const reduction = reduceSplit({
    expectedIds: ['a', 'b'],
    outcomes: [
      { childId: 'a', status: 'failed', attempt: 1 },
      { childId: 'a', status: 'completed', attempt: 2, candidateSha: 'sha-a2' },
      ok('b'),
    ],
  })
  assert.deepEqual(reduction.duplicates, ['a'])
  assert.deepEqual(reduction.duplicatedTerminal, [])
})

test('two completed outcomes for one child are named in first-seen order', () => {
  const reduction = reduceSplit({
    expectedIds: ['a', 'b'],
    outcomes: [ok('a', 1), ok('b', 1), ok('a', 2, 'sha-a2'), ok('a', 3, 'sha-a3')],
  })
  assert.deepEqual(reduction.duplicatedTerminal, ['a'])
})

test('the pre-lead_post gate reports duplicated terminal outcomes by name', () => {
  const reasons = splitJoinHoldReasons({
    expectedIds: ['a', 'b'],
    outcomes: [ok('a', 1), ok('b'), ok('a', 2, 'sha-a2')],
  })
  assert.match(reasons.join('\n'), /duplicate terminal completions: a/)
})
