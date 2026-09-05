import { test } from 'node:test'
import assert from 'node:assert/strict'
import { reduceSplit, type SplitOutcome } from '../forge/split-join'

// ENG-FORGE-HARDEN-07 — deterministic SPLIT reduce + complete join. Every
// expected child must be accounted for; missing is failure, silence never joins.

function done(childId: string, outputKeys: string[] = [`out_${childId}`], attempt = 1): SplitOutcome {
  return { childId, status: 'completed', attempt, outputKeys }
}

test('3 expected / 3 completed -> join satisfied', () => {
  const r = reduceSplit({ expectedIds: ['A', 'B', 'C'], outcomes: [done('A'), done('B'), done('C')] })
  assert.equal(r.joinSatisfied, true)
  assert.equal(r.completed.length, 3)
  assert.deepEqual(r.missing, [])
})

test('3 expected / 2 completed / 1 missing -> NO join (silence is not success)', () => {
  const r = reduceSplit({ expectedIds: ['A', 'B', 'C'], outcomes: [done('A'), done('B')] })
  assert.equal(r.joinSatisfied, false)
  assert.deepEqual(r.missing, ['C'])
})

test('3 expected / 2 completed / 1 failed -> no join, failure is visible', () => {
  const r = reduceSplit({
    expectedIds: ['A', 'B', 'C'],
    outcomes: [done('A'), done('B'), { childId: 'C', status: 'failed', attempt: 1 }],
  })
  assert.equal(r.joinSatisfied, false)
  assert.deepEqual(r.failed, ['C'])
})

test('a cancelled child is accounted but does NOT satisfy the join', () => {
  const r = reduceSplit({
    expectedIds: ['A', 'B'],
    outcomes: [done('A'), { childId: 'B', status: 'cancelled', attempt: 1 }],
  })
  assert.equal(r.joinSatisfied, false)
  assert.deepEqual(r.cancelled, ['B'])
})

test('duplicate child completion is idempotent (one accounting, not double)', () => {
  const r = reduceSplit({
    expectedIds: ['A', 'B'],
    outcomes: [done('A'), done('A'), done('B')],
  })
  assert.equal(r.accounted, 2)
  assert.equal(r.completed.length, 2)
  assert.equal(r.duplicates.length, 1)
  assert.equal(r.joinSatisfied, true)
})

test('conflicting sibling output is surfaced and blocks the join', () => {
  const r = reduceSplit({
    expectedIds: ['A', 'B'],
    outcomes: [done('A', ['routes.ts']), done('B', ['routes.ts'])],
  })
  assert.equal(r.joinSatisfied, false)
  assert.ok(r.conflicts.includes('routes.ts'))
})

test('worker retry: the later attempt wins and accounting stays correct', () => {
  const r = reduceSplit({
    expectedIds: ['A', 'B'],
    outcomes: [done('A', ['a'], 2), done('A', ['a_old'], 1), done('B')],
  })
  assert.equal(r.joinSatisfied, true)
  assert.equal(r.completed.length, 2)
})
