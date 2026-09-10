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
