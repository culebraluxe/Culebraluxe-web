import assert from 'node:assert/strict'
import test from 'node:test'

import {
  releaseState,
  sliceForBatch,
  sliceOf,
  type BatchSliceRow,
} from '../forge/forge-batch-slice'

// The batch release must see the slice it is releasing. Membership is the
// recorded deferral target — not `storyboard_story.batch`, which is display-only.
// Pure inputs, no database, per the SCOPED runtime policy.

const row = (over: Partial<BatchSliceRow> = {}): BatchSliceRow => ({
  id: 'S-1',
  batch: null,
  status: 'In Progress',
  qaPassed: null,
  publishedSha: null,
  deployedSha: null,
  productionVerified: null,
  deploymentReceipt: null,
  deploymentDeferredToBatch: null,
  ...over,
})

test('a story deferred to batch N is in slice N even when published_sha is null', () => {
  const r = row({ id: 'A', deploymentDeferredToBatch: 2 })
  assert.equal(sliceOf(r), 2)

  const slice = sliceForBatch([r], 2)
  assert.equal(slice.length, 1)
  assert.equal(slice[0].id, 'A')
  assert.equal(slice[0].batch, 2)
  assert.equal(slice[0].state, 'deferred')
})

test('a story with no deferral and nothing published belongs to no slice', () => {
  const r = row({ id: 'B' })
  assert.equal(sliceOf(r), null)
  assert.deepEqual(sliceForBatch([r], 1), [])
})

test('a story published but deferred is reported as published-and-undeployed', () => {
  const r = row({ id: 'C', deploymentDeferredToBatch: 1, publishedSha: 'abc1234' })
  const slice = sliceForBatch([r], 1)
  assert.equal(slice.length, 1)
  assert.equal(slice[0].state, 'published-and-undeployed')
  assert.equal(slice[0].publishedSha, 'abc1234')
})

test('a row with no batch number belongs to no slice — null, never a guess', () => {
  const noBatch = [null, undefined, 0, -1, Number.NaN, Number.POSITIVE_INFINITY]
  for (const deferral of noBatch) {
    const r = row({ id: 'D', batch: 3, deploymentDeferredToBatch: deferral as number | null })
    assert.equal(sliceOf(r), null)
    assert.deepEqual(sliceForBatch([r], 3), [])
    assert.deepEqual(sliceForBatch([r], 0), [])
  }
})

test('a recorded deployment outranks a deferral', () => {
  const byReceipt = row({ id: 'E', deploymentDeferredToBatch: 1, deploymentReceipt: 'vercel:123' })
  assert.equal(releaseState(byReceipt), 'deployed')

  const byVerification = row({ id: 'F', deploymentDeferredToBatch: 1, productionVerified: true })
  assert.equal(releaseState(byVerification), 'deployed')
})

test('a QA pass with no recorded deployment and no deferral is qa-passed-unrecorded', () => {
  assert.equal(releaseState(row({ id: 'G', qaPassed: true })), 'qa-passed-unrecorded')
})

test('a row with neither deferral nor QA pass nor deployment is pending', () => {
  assert.equal(releaseState(row({ id: 'H' })), 'pending')
})

test('the predicate is pure: deterministic and does not mutate its input', () => {
  const rows = [
    row({ id: 'I', deploymentDeferredToBatch: 1 }),
    row({ id: 'J', deploymentDeferredToBatch: 1, publishedSha: 'deadbee' }),
    row({ id: 'K' }),
  ]
  const snapshot = JSON.parse(JSON.stringify(rows))

  const first = sliceForBatch(rows, 1)
  const second = sliceForBatch(rows, 1)
  assert.deepEqual(first, second)
  assert.deepEqual(JSON.parse(JSON.stringify(rows)), snapshot)
  assert.deepEqual(
    first.map((e) => e.id),
    ['I', 'J'],
  )
})
