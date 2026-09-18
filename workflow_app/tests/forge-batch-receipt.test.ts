import assert from 'node:assert/strict'
import test from 'node:test'

import { recordForgeBatchReleaseReceipt } from '../../db/forge-workflow-evidence'
import {
  batchReleaseReceiptFromOutcome,
  requireBatchReleaseReceipt,
  type BatchReleaseOutcome,
} from '../forge/forge-release-receipt'

// ENG-FORGE-BATCH-RECEIPT-01 — a sprint release records what it carried.
//
// The receipt is DERIVED from the actual release outcome and cannot be written
// without one. Pure inputs, no database, per the SCOPED runtime policy: the
// writer is exercised only through its null guard, which throws before any SQL.

const SHA = 'a'.repeat(40)
const AT = '2026-09-18T10:00:00.000Z'

const outcome = (over: Partial<BatchReleaseOutcome> = {}): BatchReleaseOutcome => ({
  batch: 92,
  storyIds: ['ENG-A', 'ENG-B'],
  releasedSha: SHA,
  releasedAt: AT,
  success: true,
  ...over,
})

test('batch-receipt-from-release-outcome', () => {
  const receipt = batchReleaseReceiptFromOutcome(outcome())
  assert.ok(receipt, 'a real release outcome yields a receipt')
  assert.equal(receipt!.batch, 92)
  assert.deepEqual(receipt!.storyIds, ['ENG-A', 'ENG-B'])
  assert.equal(receipt!.releasedSha, SHA)
  assert.equal(receipt!.releasedAt, AT)

  // duplicate story ids are one carried story, not two
  assert.deepEqual(
    batchReleaseReceiptFromOutcome(outcome({ storyIds: ['ENG-A', 'ENG-A'] }))!.storyIds,
    ['ENG-A'],
  )
})

test('no-release-outcome-yields-no-receipt', () => {
  const noRelease: Array<BatchReleaseOutcome | null | undefined> = [
    null,
    undefined,
    outcome({ success: false }),
    outcome({ releasedSha: null }),
    outcome({ releasedSha: 'not-a-sha' }),
    outcome({ batch: null }),
    outcome({ batch: 0 }),
    outcome({ storyIds: [] }),
    outcome({ releasedAt: null }),
  ]
  for (const candidate of noRelease) {
    assert.equal(batchReleaseReceiptFromOutcome(candidate), null, JSON.stringify(candidate))
  }
})

test('receipt-write-refused-without-outcome', async () => {
  // the record path's guard refuses to turn an absent outcome into a receipt
  assert.throws(() => requireBatchReleaseReceipt(null))
  assert.throws(() => requireBatchReleaseReceipt(outcome({ releasedSha: null })))

  // the writer refuses to write without a derived receipt — before any SQL
  await assert.rejects(() => recordForgeBatchReleaseReceipt(null))
  await assert.rejects(() => recordForgeBatchReleaseReceipt(undefined))
})
