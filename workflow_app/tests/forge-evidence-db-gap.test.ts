import assert from 'node:assert/strict'
import { test } from 'node:test'
import { mapRunsToGateEvidence, type ForgeRunRowShape } from '../forge/forge-evidence-db'

const qaRow = (over: Partial<ForgeRunRowShape>): ForgeRunRowShape => ({
  run_type: 'qa',
  result_status: 'failed',
  commit_hash: null,
  ...over,
})

test('evidence mapping: a config-gap qa failure (MISSING_ASSAY_PLAN) sets verificationGap, not a repair', () => {
  const ev = mapRunsToGateEvidence([qaRow({ failure_code: 'MISSING_ASSAY_PLAN' })])
  assert.equal(ev.qaPassed, false)
  assert.equal(ev.verificationGap, true)
})

test('evidence mapping: ASSAY_POLICY_FAILED is a verification gap; a real test failure is not', () => {
  const policy = mapRunsToGateEvidence([qaRow({ failure_code: 'ASSAY_POLICY_FAILED' })])
  assert.equal(policy.verificationGap, true)
  const realFail = mapRunsToGateEvidence([qaRow({ failure_code: 'ASSAY_TEST_FAILED' })])
  assert.equal(realFail.verificationGap, undefined, 'a real test failure is repairable, not a gap')
  assert.equal(realFail.qaPassed, false)
})

test('evidence mapping: a clean qa run is not a gap; missing failure_code never fabricates a gap', () => {
  const clean = mapRunsToGateEvidence([qaRow({ result_status: 'complete', failure_code: null })])
  assert.equal(clean.verificationGap, undefined)
  assert.equal(mapRunsToGateEvidence([qaRow({ failure_code: null })]).verificationGap, undefined)
})

