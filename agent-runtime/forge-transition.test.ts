import assert from 'node:assert/strict'
import test from 'node:test'

import { decideForgeTransition } from './forge-transition'

const CANDIDATE = 'a'.repeat(40)

test('Architect hands a frozen contract to Lead PRE', () => {
  const decision = decideForgeTransition({ type: 'architect-complete' })
  assert.equal(decision.action, 'enqueue-lead')
  assert.equal(decision.nextLane, 'lead')
  assert.equal(decision.nextPhase, 'pre')
})

test('Lead PRE chooses cheapest sound execution shape', () => {
  const solo = decideForgeTransition({ type: 'lead-pre', decision: 'SOLO' })
  assert.equal(solo.action, 'enqueue-lead')
  assert.equal(solo.nextPhase, 'implement')

  const smith = decideForgeTransition({ type: 'lead-pre', decision: 'SMITH' })
  assert.equal(smith.action, 'enqueue-smith')
  assert.equal(smith.nextLane, 'smith')
})

test('Lead may veto architecture before implementation', () => {
  const decision = decideForgeTransition({
    type: 'lead-pre',
    decision: 'HOLD',
    detail: 'scope conflicts with repository reality',
  })
  assert.equal(decision.action, 'hold-human')
  assert.equal(decision.failure?.code, 'LEAD_ARCHITECTURE_CHALLENGE')
})

test('Lead split is preserved but fails closed until multi-worker exists', () => {
  const decision = decideForgeTransition({
    type: 'lead-pre',
    decision: 'SPLIT',
    splitCount: 3,
  })
  assert.equal(decision.action, 'hold-human')
  assert.equal(decision.failure?.code, 'LEAD_SPLIT_REQUIRES_MULTIWORKER')
})

test('Smith candidate goes to Lead POST, not directly to Assay', () => {
  const decision = decideForgeTransition({
    type: 'smith-complete',
    candidateSha: CANDIDATE,
  })
  assert.equal(decision.action, 'enqueue-lead')
  assert.equal(decision.nextLane, 'lead')
  assert.equal(decision.nextPhase, 'post')
})

test('Lead SOLO implementation and approved POST both hand exact candidate to Assay', () => {
  const solo = decideForgeTransition({
    type: 'lead-implement-complete',
    candidateSha: CANDIDATE,
  })
  assert.equal(solo.action, 'enqueue-assay')

  const post = decideForgeTransition({
    type: 'lead-post',
    decision: 'ASSAY',
    candidateSha: CANDIDATE,
  })
  assert.equal(post.action, 'enqueue-assay')
})

test('Lead POST may stop when implementation exposes architecture defect', () => {
  const decision = decideForgeTransition({
    type: 'lead-post',
    decision: 'HOLD',
    candidateSha: CANDIDATE,
    detail: 'integration exposed incompatible architecture',
  })
  assert.equal(decision.action, 'hold-human')
  assert.equal(decision.failure?.code, 'LEAD_INTEGRATION_FAILED')
})

test('missing candidate stops before downstream verification', () => {
  const smith = decideForgeTransition({ type: 'smith-complete', candidateSha: null })
  assert.equal(smith.action, 'hold-human')
  assert.equal(smith.failure?.code, 'NO_CANDIDATE')

  const lead = decideForgeTransition({
    type: 'lead-implement-complete',
    candidateSha: null,
  })
  assert.equal(lead.action, 'hold-human')
  assert.equal(lead.failure?.code, 'NO_CANDIDATE')
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
})

test('publish conflict preserves human gate; successful publish alone completes story', () => {
  const conflict = decideForgeTransition({
    type: 'publish-conflict',
    detail: 'main advanced',
  })
  assert.equal(conflict.action, 'hold-human')

  const complete = decideForgeTransition({ type: 'publish-complete' })
  assert.equal(complete.action, 'complete')
  assert.equal(complete.storyStatus, 'Complete')
})
