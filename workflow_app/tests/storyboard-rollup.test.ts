import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  WORKSTREAMS,
  buildStoryBoardModel,
  statusBucket,
  type StoryRecord,
} from '../../lib/storyboard-data'

// Pure-model tests for the rollup: completion math uses the stored completion
// (0..100), status is categorical, buckets feed the counts, and rollup=false
// parents are excluded.

function story(overrides: Partial<StoryRecord> & { id: string }): StoryRecord {
  return {
    id: overrides.id,
    workstream: overrides.workstream ?? 'CRM',
    title: overrides.title ?? `Story ${overrides.id}`,
    priority: overrides.priority ?? 'Medium',
    status: overrides.status ?? 'Planned',
    notes: overrides.notes ?? '',
    batch: overrides.batch ?? null,
    goal: overrides.goal ?? null,
    scope: overrides.scope ?? null,
    dependencies: overrides.dependencies ?? null,
    preconditions: overrides.preconditions ?? null,
    architectBrief: overrides.architectBrief ?? null,
    contextRefs: overrides.contextRefs ?? null,
    acceptanceCriteria: overrides.acceptanceCriteria ?? null,
    postconditions: overrides.postconditions ?? null,
    architectBriefUpdatedAt: overrides.architectBriefUpdatedAt ?? null,
    // Complete forces 100; otherwise the caller provides an honest 0..100.
    completion:
      overrides.completion ?? (overrides.status === 'Complete' ? 100 : 0),
    rollup: overrides.rollup ?? true,
    plannedStartAt: overrides.plannedStartAt ?? null,
    actualStartAt: overrides.actualStartAt ?? null,
    completedAt: overrides.completedAt ?? null,
    createdAt: overrides.createdAt ?? '2026-08-21T00:00:00Z',
    updatedAt: overrides.updatedAt ?? '2026-08-21T00:00:00Z',
  }
}

test('statusBucket categorizes the eight controlled statuses', () => {
  assert.equal(statusBucket('Complete'), 'complete')
  assert.equal(statusBucket('In Progress'), 'partial')
  assert.equal(statusBucket('Partial'), 'partial')
  assert.equal(statusBucket('Planned'), 'open')
  assert.equal(statusBucket('Deferred'), 'open')
  assert.equal(statusBucket('Hold'), 'open')
  assert.equal(statusBucket('Failed'), 'open')
  assert.equal(statusBucket('Blocked'), 'blocked')
})

test('completion math uses the stored completion, not the status', () => {
  // Same completion, different statuses → identical workstream completion.
  const model = buildStoryBoardModel([
    story({ id: 'A-1', workstream: 'CRM', status: 'Complete', completion: 100 }),
    story({ id: 'A-2', workstream: 'CRM', status: 'Partial', completion: 100 }),
    story({ id: 'A-3', workstream: 'CRM', status: 'Blocked', completion: 50 }),
    story({ id: 'A-4', workstream: 'CRM', status: 'Planned', completion: 50 }),
  ])
  const crm = model.workstreams.find((w) => w.code === 'CRM')!
  assert.equal(crm.completionPercent, 75) // (100 + 100 + 50 + 50) / 4
  assert.equal(crm.completeCount, 1)
  assert.equal(crm.partialCount, 1)
  assert.equal(crm.openCount, 1)
  assert.equal(crm.blockedCount, 1)
})

test('workstream completion is the AVG of the stored completion', () => {
  const model = buildStoryBoardModel([
    story({ id: 'A-1', workstream: 'TXN', status: 'Partial', completion: 100 }),
    story({ id: 'A-2', workstream: 'TXN', status: 'Planned', completion: 50 }),
    story({ id: 'A-3', workstream: 'TXN', status: 'Blocked', completion: 0 }),
  ])
  const txn = model.workstreams.find((w) => w.code === 'TXN')!
  assert.equal(txn.storyCount, 3)
  assert.equal(txn.completionPercent, 50) // (100 + 50 + 0) / 3
})

test('rollup=false parents are stored but excluded from rollup counts', () => {
  const model = buildStoryBoardModel([
    story({ id: 'P-1', workstream: 'TXN', status: 'Complete', rollup: false }),
    story({ id: 'C-1', workstream: 'TXN', status: 'Partial', completion: 100 }),
    story({ id: 'C-2', workstream: 'TXN', status: 'Planned', completion: 50 }),
  ])
  const txn = model.workstreams.find((w) => w.code === 'TXN')!
  assert.equal(txn.storedCount, 3)
  assert.equal(txn.storyCount, 2)
  assert.equal(txn.completeCount, 0)
  assert.equal(txn.partialCount, 1)
  assert.equal(txn.completionPercent, 75) // (100 + 50) / 2 over rollup only
})

test('net-net sums workstream completion x weight', () => {
  const model = buildStoryBoardModel([
    // PUBLIC has 20% weight and 100% completion.
    story({ id: 'X-1', workstream: 'PUBLIC', status: 'Complete' }),
    // CRM has 20% weight and 50% completion.
    story({ id: 'Y-1', workstream: 'CRM', status: 'Partial', completion: 50 }),
  ])
  // 100 * 0.20 + 50 * 0.20 = 30
  assert.equal(model.netNet, 30)
})

test('changing only completion changes workstream completion and net-net', () => {
  const before = buildStoryBoardModel([
    story({ id: 'Z-1', workstream: 'CRM', status: 'Partial', completion: 20 }),
  ])
  const after = buildStoryBoardModel([
    story({ id: 'Z-1', workstream: 'CRM', status: 'Partial', completion: 80 }),
  ])
  const b = before.workstreams.find((w) => w.code === 'CRM')!
  const a = after.workstreams.find((w) => w.code === 'CRM')!
  assert.equal(b.completionPercent, 20)
  assert.equal(a.completionPercent, 80)
  assert.notEqual(after.netNet, before.netNet)
})

test('changing status without changing completion does not alter completion math', () => {
  const planned = buildStoryBoardModel([
    story({ id: 'Z-1', workstream: 'CRM', status: 'Planned', completion: 40 }),
  ])
  const blocked = buildStoryBoardModel([
    story({ id: 'Z-1', workstream: 'CRM', status: 'Blocked', completion: 40 }),
  ])
  const p = planned.workstreams.find((w) => w.code === 'CRM')!
  const b = blocked.workstreams.find((w) => w.code === 'CRM')!
  assert.equal(p.completionPercent, 40)
  assert.equal(b.completionPercent, 40)
  assert.equal(planned.netNet, blocked.netNet)
  // Only the count buckets change.
  assert.equal(p.openCount, 1)
  assert.equal(b.blockedCount, 1)
})

test('weights reconcile to 100 across the eight workstreams', () => {
  assert.equal(WORKSTREAMS.length, 8)
  assert.equal(WORKSTREAMS.reduce((sum, w) => sum + w.weight, 0), 100)
})
