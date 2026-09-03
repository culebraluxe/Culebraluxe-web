import { test } from 'node:test'
import assert from 'node:assert/strict'
import { pickLane } from './orchestrate'

test('no brief → scout', () => {
  assert.equal(pickLane({ story: {} }), 'scout')
})

test('brief present → smith', () => {
  assert.equal(pickLane({ story: { architectBrief: 'do the thing' } }), 'smith')
})

test('after builder → assay', () => {
  assert.equal(
    pickLane({ story: { architectBrief: 'x' }, lastFinishedRole: 'builder' }),
    'assay',
  )
})

test('after scout → smith', () => {
  assert.equal(
    pickLane({
      story: { architectBrief: 'x' },
      lastFinishedRole: 'scout',
    }),
    'smith',
  )
})
