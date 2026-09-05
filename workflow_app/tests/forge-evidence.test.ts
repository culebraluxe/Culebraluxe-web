import assert from 'node:assert/strict'
import test from 'node:test'

import { mapRunsToGateEvidence, type ForgeRunRowShape } from '../forge/forge-evidence-db'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 #1 — durable run-table -> gate evidence mapping (pure).
// ---------------------------------------------------------------------------

test('ENG-FORGE-V9: a clean QA run maps to qaPassed true', () => {
  const rows: ForgeRunRowShape[] = [
    { run_type: 'qa', result_status: 'complete', commit_hash: null },
  ]
  const ev = mapRunsToGateEvidence(rows)
  assert.equal(ev.qaPassed, true)
})

test('ENG-FORGE-V9: clean publish + deploy + smoke runs map to their gates', () => {
  const ev = mapRunsToGateEvidence([
    { run_type: 'publish', result_status: 'complete', commit_hash: 'abc123' },
    { run_type: 'deploy', result_status: 'success', commit_hash: null },
    { run_type: 'production_smoke', result_status: 'pass', commit_hash: null },
  ])
  assert.equal(ev.publishSucceeded, true)
  assert.equal(ev.deploymentSucceeded, true)
  assert.equal(ev.productionVerified, true)
})

test('ENG-FORGE-V9: a failed QA run stays qaPassed false (never advances QA gate)', () => {
  const ev = mapRunsToGateEvidence([
    { run_type: 'qa', result_status: 'failed', commit_hash: null },
  ])
  assert.equal(ev.qaPassed, false)
})

test('ENG-FORGE-V9: unrelated/no rows produce no fabricated gate facts', () => {
  const ev = mapRunsToGateEvidence([
    { run_type: 'smith', result_status: 'complete', commit_hash: 'abc' },
  ])
  assert.equal(ev.qaPassed, undefined)
  assert.equal(ev.publishSucceeded, undefined)
})
