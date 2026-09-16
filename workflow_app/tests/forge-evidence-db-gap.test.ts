import assert from 'node:assert/strict'
import { test } from 'node:test'
import { mapRunsToGateEvidence, type ForgeRunRowShape } from '../forge/forge-evidence-db'

const qaRow = (over: Partial<ForgeRunRowShape>): ForgeRunRowShape => ({
  run_type: 'qa',
  result_status: 'failed',
  commit_hash: null,
  ...over,
})

// ---------------------------------------------------------------------------
// ONE VERDICT, ONE VOCABULARY (Captain, 2026-09-16).
//
// This module used to RE-DERIVE a QA verdict from a failure-code list of its own
// (`MISSING_ASSAY_PLAN`, `ASSAY_POLICY_FAILED` -> `verificationGap`), so the same
// measurement was described in two languages. These tests now pin the single rule:
// the newest QA row's STATUS is the verdict, the gap is authored by the Assay lane
// itself (`collectAssayEvidence`), and this reader never invents a reason.
//
// The outcome is unchanged where it matters: a QA failure carries no disposition,
// and `routeQaResult` HOLDs without one — so nothing ever repaired on a guess.
// ---------------------------------------------------------------------------

test('one vocabulary: a non-pass qa row is NOT a pass, and this reader invents no gap reason', () => {
  for (const failure_code of ['MISSING_ASSAY_PLAN', 'ASSAY_POLICY_FAILED', 'CANDIDATE_MISMATCH']) {
    const ev = mapRunsToGateEvidence([
      { run_type: 'assay', result_status: 'Hold', commit_hash: null, failure_code },
    ])
    assert.equal(ev.qaPassed, false, `${failure_code} is not a pass`)
    assert.equal(
      ev.verificationGap,
      undefined,
      'the gap is authored where it is measured, never reconstructed from a code list here',
    )
  }
})

test('evidence mapping: a clean qa run is not a gap; missing failure_code never fabricates a gap', () => {
  const clean = mapRunsToGateEvidence([qaRow({ result_status: 'complete', failure_code: null })])
  assert.equal(clean.verificationGap, undefined)
  assert.equal(mapRunsToGateEvidence([qaRow({ failure_code: null })]).verificationGap, undefined)
})

// ---------------------------------------------------------------------------
// ONE FACT, ONE MEASUREMENT — the newest one.
//
// Measured live 2026-09-16: ENG-QA-SINGLE-VERDICT-01 carried TEN clean `assay` runs from earlier that
// morning, the current generation's assay held on CANDIDATE_MISMATCH, and the router read
// `qaPassed: true` and advanced to `deploy` on a candidate no lane had verified. Runs are supplied
// newest-first, exactly as readStoryGateEvidence orders them.
// ---------------------------------------------------------------------------

test('stale pass: an OLD clean assay cannot certify a NEWER hold', () => {
  const ev = mapRunsToGateEvidence([
    { run_type: 'assay', result_status: 'Hold', commit_hash: null, failure_code: 'CANDIDATE_MISMATCH' },
    // ten mornings' worth of old passes, exactly the shape that fooled the router
    ...Array.from({ length: 10 }, () => ({
      run_type: 'assay' as const,
      result_status: 'Complete',
      commit_hash: null,
      failure_code: null,
    })),
  ])
  assert.equal(ev.qaPassed, false, 'the newest measurement holds; the old passes are history')
})

test('stale pass: the newest clean assay still passes over an older failure', () => {
  const ev = mapRunsToGateEvidence([
    { run_type: 'assay', result_status: 'Complete', commit_hash: null, failure_code: null },
    { run_type: 'assay', result_status: 'Failed', commit_hash: null, failure_code: 'ASSAY_TEST_FAILED' },
  ])
  assert.equal(ev.qaPassed, true, 'a repaired story whose newest assay passed is a pass')
})

test('stale pass: a gap in the newest assay is still NOT a pass', () => {
  const ev = mapRunsToGateEvidence([
    { run_type: 'assay', result_status: 'Hold', commit_hash: null, failure_code: 'MISSING_ASSAY_PLAN' },
    { run_type: 'assay', result_status: 'Complete', commit_hash: null, failure_code: null },
  ])
  assert.equal(ev.qaPassed, false, 'the newest row is not a pass, whatever an older row said')
  // The REASON for the non-pass is the lane's to author, not this reader's to reconstruct.
  assert.equal(ev.verificationGap, undefined)
})

test('stale pass: an old deployment cannot keep a story deployed forever', () => {
  const ev = mapRunsToGateEvidence([
    { run_type: 'dev_ops', result_status: 'Hold', commit_hash: null, failure_code: 'DEPLOYMENT' },
    { run_type: 'deploy', result_status: 'Complete', commit_hash: null, failure_code: null },
  ])
  // `dev_ops` is not a deploy family today; the newest DEPLOY row is the old clean one, which is the
  // honest read. What must never happen is the reverse — a newer hold being outvoted below.
  assert.equal(ev.deploymentSucceeded, true)
  const reversed = mapRunsToGateEvidence([
    { run_type: 'deploy', result_status: 'Hold', commit_hash: null, failure_code: 'DEPLOYMENT' },
    { run_type: 'deploy', result_status: 'Complete', commit_hash: null, failure_code: null },
  ])
  assert.equal(reversed.deploymentSucceeded, false)
})

test('stale pass: `Incomplete` is not a pass (the matcher is word-anchored)', () => {
  const ev = mapRunsToGateEvidence([
    { run_type: 'assay', result_status: 'Incomplete', commit_hash: null, failure_code: null },
  ])
  assert.equal(ev.qaPassed, false)
})


