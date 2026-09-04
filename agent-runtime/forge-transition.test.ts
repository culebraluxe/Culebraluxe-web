import assert from 'node:assert/strict'
import test from 'node:test'

import { decideForgeTransition } from './forge-transition'

const CANDIDATE = 'a'.repeat(40)

test('Smith candidate advances only to Assay', () => {
  assert.deepEqual(
    decideForgeTransition({ type: 'smith-complete', candidateSha: CANDIDATE }),
    {
      action: 'enqueue-assay',
      nextLane: 'assay',
      storyStatus: null,
      humanRequired: false,
      failure: null,
    },
  )
})

test('Smith without candidate stops for human', () => {
  const decision = decideForgeTransition({
    type: 'smith-complete',
    candidateSha: null,
  })
  assert.equal(decision.action, 'hold-human')
  assert.equal(decision.humanRequired, true)
  assert.equal(decision.failure?.code, 'NO_CANDIDATE')
})

test('Assay PASS advances to publish, not directly to Complete', () => {
  const decision = decideForgeTransition({ type: 'assay-pass' })
  assert.equal(decision.action, 'publish')
  assert.equal(decision.storyStatus, null)
  assert.equal(decision.humanRequired, false)
})

test('Assay FAIL is a hard human boundary and never returns Smith', () => {
  const decision = decideForgeTransition({
    type: 'assay-fail',
    code: 'ASSAY_TEST_FAILED',
    detail: '1 test failed',
  })
  assert.equal(decision.action, 'hold-human')
  assert.equal(decision.nextLane, null)
  assert.equal(decision.storyStatus, 'Hold')
  assert.equal(decision.humanRequired, true)
  assert.equal(decision.failure?.code, 'ASSAY_TEST_FAILED')
})

test('Assay interruption is also human-owned regardless of retry budget elsewhere', () => {
  const decision = decideForgeTransition({
    type: 'assay-runtime-interrupted',
    detail: 'worker died',
  })
  assert.equal(decision.action, 'hold-human')
  assert.equal(decision.nextLane, null)
  assert.equal(decision.failure?.code, 'ASSAY_RUNTIME_INTERRUPTED')
})

test('Smith infrastructure interruption may retry only within budget', () => {
  const retry = decideForgeTransition({
    type: 'smith-runtime-interrupted',
    attempts: 1,
    maxAttempts: 3,
    detail: 'transient runtime failure',
  })
  assert.equal(retry.action, 'retry-same-lane')
  assert.equal(retry.humanRequired, false)

  const exhausted = decideForgeTransition({
    type: 'smith-runtime-interrupted',
    attempts: 3,
    maxAttempts: 3,
    detail: 'runtime still failing',
  })
  assert.equal(exhausted.action, 'hold-human')
  assert.equal(exhausted.humanRequired, true)
})

test('publish conflict preserves human gate; successful publish alone completes story', () => {
  const conflict = decideForgeTransition({
    type: 'publish-conflict',
    detail: 'main advanced',
  })
  assert.equal(conflict.action, 'hold-human')
  assert.equal(conflict.storyStatus, 'Hold')

  const complete = decideForgeTransition({ type: 'publish-complete' })
  assert.equal(complete.action, 'complete')
  assert.equal(complete.storyStatus, 'Complete')
})
