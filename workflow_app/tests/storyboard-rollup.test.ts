import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  WORKSTREAMS,
  buildStoryBoardModel,
  statusBucket,
  statusScore,
  type StoryRecord,
} from '../../lib/storyboard-data'

// Pure-model tests for the 8/21 rollup: status scoring, buckets, per-workstream
// completion, net-net, and rollup=false parent exclusion.

function story(overrides: Partial<StoryRecord> & { id: string }): StoryRecord {
  return {
    id: overrides.id,
    workstream: overrides.workstream ?? 'CRM',
    title: overrides.title ?? `Story ${overrides.id}`,
    priority: overrides.priority ?? 'Medium',
    status: overrides.status ?? 'Open',
    notes: overrides.notes ?? '',
    batch: overrides.batch ?? null,
    goal: overrides.goal ?? null,
    scope: overrides.scope ?? null,
    acceptanceCriteria: overrides.acceptanceCriteria ?? null,
    dependencies: overrides.dependencies ?? null,
    completion: overrides.completion ?? 0,
    rollup: overrides.rollup ?? true,
  }
}

test('statusScore maps every scoring tier', () => {
  assert.equal(statusScore('Complete'), 1)
  assert.equal(statusScore('Complete V1'), 1)
  assert.equal(statusScore('Complete V2'), 1)
  assert.equal(statusScore('Operationalized'), 1)
  assert.equal(statusScore('Operationalized V1'), 1)
  assert.equal(statusScore('Operationalized V2'), 1)
  assert.equal(statusScore('Read-side complete'), 0.8)
  assert.equal(statusScore('Read-side V1'), 0.8)
  assert.equal(statusScore('Read-side V1 complete'), 0.8)
  assert.equal(statusScore('Readiness PASS'), 0.8)
  assert.equal(statusScore('Partial'), 0.5)
  assert.equal(statusScore('strong V1 core'), 0.5)
  assert.equal(statusScore('Minor remainder'), 0.5)
  assert.equal(statusScore('Browser-local V1'), 0.5)
  assert.equal(statusScore('Planned'), 0)
  assert.equal(statusScore('Open'), 0)
  assert.equal(statusScore('Blocked'), 0)
  assert.equal(statusScore('Deferred'), 0)
  assert.equal(statusScore('Hardware/content dependent'), 0)
})

test('statusBucket categorizes the count buckets', () => {
  assert.equal(statusBucket('Complete'), 'complete')
  assert.equal(statusBucket('Operationalized V2'), 'complete')
  assert.equal(statusBucket('Read-side V1'), 'partial')
  assert.equal(statusBucket('strong V1 core'), 'partial')
  assert.equal(statusBucket('Planned'), 'open')
  assert.equal(statusBucket('Deferred'), 'open')
  assert.equal(statusBucket('Hardware/content dependent'), 'open')
  assert.equal(statusBucket('Blocked'), 'blocked')
})

test('workstream completion is the mean status score of its rollup stories', () => {
  const model = buildStoryBoardModel([
    story({ id: 'A-1', workstream: 'CRM', status: 'Complete' }),
    story({ id: 'A-2', workstream: 'CRM', status: 'Partial' }),
    story({ id: 'A-3', workstream: 'CRM', status: 'Blocked' }),
    story({ id: 'A-4', workstream: 'CRM', status: 'Planned' }),
  ])
  const crm = model.workstreams.find((w) => w.code === 'CRM')!
  assert.equal(crm.storyCount, 4)
  assert.equal(crm.completeCount, 1)
  assert.equal(crm.partialCount, 1)
  assert.equal(crm.openCount, 1)
  assert.equal(crm.blockedCount, 1)
  assert.equal(crm.completionPercent, 37.5) // (1 + 0.5 + 0 + 0) / 4
})

test('rollup=false parents are stored but excluded from rollup counts', () => {
  const model = buildStoryBoardModel([
    story({ id: 'P-1', workstream: 'TXN', status: 'Complete', rollup: false }),
    story({ id: 'C-1', workstream: 'TXN', status: 'Complete', rollup: true }),
    story({ id: 'C-2', workstream: 'TXN', status: 'Partial', rollup: true }),
  ])
  const txn = model.workstreams.find((w) => w.code === 'TXN')!
  assert.equal(txn.storedCount, 3)
  assert.equal(txn.storyCount, 2)
  assert.equal(txn.completeCount, 1)
  assert.equal(txn.partialCount, 1)
  assert.equal(txn.completionPercent, 75) // (1 + 0.5) / 2
})

test('net-net sums workstream completion x weight', () => {
  const model = buildStoryBoardModel([
    // PUBLIC has 20% weight and 100% completion.
    story({ id: 'X-1', workstream: 'PUBLIC', status: 'Complete' }),
    // CRM has 20% weight and 50% completion.
    story({ id: 'Y-1', workstream: 'CRM', status: 'Partial' }),
  ])
  // 100 * 0.20 + 50 * 0.20 = 30
  assert.equal(model.netNet, 30)
})

test('changing a status changes workstream completion and net-net', () => {
  const planned = buildStoryBoardModel([
    story({ id: 'Z-1', workstream: 'CRM', status: 'Planned' }),
  ])
  const completed = buildStoryBoardModel([
    story({ id: 'Z-1', workstream: 'CRM', status: 'Complete' }),
  ])
  const before = planned.workstreams.find((w) => w.code === 'CRM')!
  const after = completed.workstreams.find((w) => w.code === 'CRM')!
  assert.equal(before.completionPercent, 0)
  assert.equal(after.completionPercent, 100)
  assert.notEqual(after.completionPercent, before.completionPercent)
  assert.notEqual(completed.netNet, planned.netNet)
})

test('weights reconcile to 100 across the eight workstreams', () => {
  assert.equal(WORKSTREAMS.length, 8)
  assert.equal(WORKSTREAMS.reduce((sum, w) => sum + w.weight, 0), 100)
})
