import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  NEXT_WORK_DEFAULT_LIMIT,
  NEXT_WORK_MAX_LIMIT,
  dependencyStoryIds,
  isStoryActionable,
  priorityRankOf,
  selectNextWork,
  type StoryRecord,
} from '../../lib/storyboard-data'

// OPS-08 — Story Board Batch / Next Work Selection. Pure-model tests for the
// bounded, deterministic "Next 20" projection: eligibility (rollup +
// actionable status + satisfied dependencies), deterministic ordering
// (batch → priority → planned start → id), the cap, and the held-back count.
// Zero Neon, zero React.

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

const ids = (selection: ReturnType<typeof selectNextWork>): string[] =>
  selection.entries.map((entry) => entry.story.id)

test('priorityRankOf matches the ENG-16 priority ladder', () => {
  assert.equal(priorityRankOf('Critical'), 0)
  assert.equal(priorityRankOf('High'), 1)
  assert.equal(priorityRankOf('High-ish'), 2)
  assert.equal(priorityRankOf('Medium-High'), 3)
  assert.equal(priorityRankOf('Medium'), 4)
  assert.equal(priorityRankOf('Low'), 5)
  assert.equal(priorityRankOf('Later'), 6)
  assert.equal(priorityRankOf('High-value polish'), 7)
  assert.equal(priorityRankOf('Unknown-Priority'), 99)
})

test('isStoryActionable requires rollup and an actionable status', () => {
  assert.equal(isStoryActionable(story({ id: 'A-1', status: 'Planned' })), true)
  assert.equal(isStoryActionable(story({ id: 'A-2', status: 'Ready' })), true)
  assert.equal(isStoryActionable(story({ id: 'A-3', status: 'In Progress' })), true)
  assert.equal(isStoryActionable(story({ id: 'A-4', status: 'Partial' })), true)
  assert.equal(isStoryActionable(story({ id: 'A-5', status: 'Complete' })), false)
  assert.equal(isStoryActionable(story({ id: 'A-6', status: 'Blocked' })), false)
  assert.equal(isStoryActionable(story({ id: 'A-7', status: 'Failed' })), false)
  assert.equal(isStoryActionable(story({ id: 'A-8', status: 'Deferred' })), false)
  assert.equal(isStoryActionable(story({ id: 'A-9', status: 'Hold' })), false)
  // Reference / parent rows are never selected as work.
  assert.equal(
    isStoryActionable(story({ id: 'A-10', status: 'Planned', rollup: false })),
    false,
  )
})

test('dependencyStoryIds extracts story-ID-like tokens, deduped, case-insensitive', () => {
  assert.deepEqual(dependencyStoryIds(null), [])
  assert.deepEqual(dependencyStoryIds(''), [])
  assert.deepEqual(dependencyStoryIds('no ids here'), [])
  assert.deepEqual(dependencyStoryIds('requires CRM-14B and OPS-07'), [
    'CRM-14B',
    'OPS-07',
  ])
  assert.deepEqual(dependencyStoryIds('crm-14b then CRM-14B again'), ['CRM-14B'])
  assert.deepEqual(dependencyStoryIds('needs DOC-01 after PX-24'), [
    'DOC-01',
    'PX-24',
  ])
  // Plain dates/numbers are not story IDs.
  assert.deepEqual(dependencyStoryIds('after 2026-08-21 M-1'), [])
})

test('non-actionable statuses are excluded from Next Work', () => {
  const selection = selectNextWork([
    story({ id: 'PLANNED', status: 'Planned' }),
    story({ id: 'READY', status: 'Ready' }),
    story({ id: 'INPROG', status: 'In Progress' }),
    story({ id: 'PARTIAL', status: 'Partial' }),
    story({ id: 'DONE', status: 'Complete' }),
    story({ id: 'BLOCKED', status: 'Blocked' }),
    story({ id: 'FAILED', status: 'Failed' }),
    story({ id: 'DEFERRED', status: 'Deferred' }),
    story({ id: 'HOLD', status: 'Hold' }),
  ])
  // All four are equal on batch/priority/planned start, so the documented
  // final tiebreak (id ascending) decides the order.
  assert.deepEqual(ids(selection), ['INPROG', 'PARTIAL', 'PLANNED', 'READY'])
  assert.equal(selection.totalEligible, 4)
  assert.equal(selection.totalBlockedByDependency, 0)
  assert.equal(selection.truncated, false)
})

test('rollup=false reference rows are never selected', () => {
  const selection = selectNextWork([
    story({ id: 'PARENT', status: 'Planned', rollup: false }),
    story({ id: 'CHILD-1', status: 'Planned' }),
  ])
  assert.deepEqual(ids(selection), ['CHILD-1'])
  assert.equal(selection.totalEligible, 1)
})

test('default limit is 20 and truncation is reported', () => {
  assert.equal(NEXT_WORK_DEFAULT_LIMIT, 20)
  const board = Array.from({ length: 25 }, (_, i) =>
    story({ id: `S-${String(i).padStart(2, '0')}` }),
  )
  const selection = selectNextWork(board)
  assert.equal(selection.entries.length, 20)
  assert.equal(selection.totalEligible, 25)
  assert.equal(selection.truncated, true)
  assert.equal(selection.limit, 20)
  // Ranks are 1-based and sequential.
  assert.deepEqual(
    selection.entries.map((e) => e.rank),
    Array.from({ length: 20 }, (_, i) => i + 1),
  )
})

test('limit is clamped to 1..NEXT_WORK_MAX_LIMIT and invalid values default', () => {
  const board = Array.from({ length: 60 }, (_, i) =>
    story({ id: `S-${String(i).padStart(2, '0')}` }),
  )
  assert.equal(selectNextWork(board, { limit: 0 }).limit, 1)
  assert.equal(selectNextWork(board, { limit: -5 }).limit, 1)
  assert.equal(selectNextWork(board, { limit: 5 }).limit, 5)
  assert.equal(selectNextWork(board, { limit: 999 }).limit, NEXT_WORK_MAX_LIMIT)
  assert.equal(selectNextWork(board, { limit: NaN }).limit, NEXT_WORK_DEFAULT_LIMIT)
  assert.equal(selectNextWork(board, { limit: undefined }).limit, NEXT_WORK_DEFAULT_LIMIT)
  assert.equal(selectNextWork(board).limit, NEXT_WORK_DEFAULT_LIMIT)
})

test('ordering is by priority rank (Critical first, unknown last)', () => {
  const selection = selectNextWork([
    story({ id: 'LOW', priority: 'Low' }),
    story({ id: 'UNKNOWN', priority: 'Whatever' }),
    story({ id: 'CRIT', priority: 'Critical' }),
    story({ id: 'HIGH', priority: 'High' }),
  ])
  assert.deepEqual(ids(selection), ['CRIT', 'HIGH', 'LOW', 'UNKNOWN'])
})

test('batch orders before priority: ascending batches first, unbatched last', () => {
  const selection = selectNextWork([
    story({ id: 'B5', batch: 5, priority: 'Critical' }),
    story({ id: 'NOBATCH', priority: 'Critical' }),
    story({ id: 'B1', batch: 1, priority: 'Low' }),
    story({ id: 'B2', batch: 2, priority: 'High' }),
  ])
  assert.deepEqual(ids(selection), ['B1', 'B2', 'B5', 'NOBATCH'])
})

test('planned start breaks priority ties; unplanned sorts last', () => {
  const selection = selectNextWork([
    story({ id: 'LATER', priority: 'High', plannedStartAt: '2026-09-01' }),
    story({ id: 'UNPLANNED', priority: 'High', plannedStartAt: null }),
    story({ id: 'EARLIER', priority: 'High', plannedStartAt: '2026-08-25' }),
  ])
  assert.deepEqual(ids(selection), ['EARLIER', 'LATER', 'UNPLANNED'])
})

test('id is the final deterministic tiebreak', () => {
  const selection = selectNextWork([
    story({ id: 'Z-2', priority: 'Medium' }),
    story({ id: 'A-1', priority: 'Medium' }),
    story({ id: 'M-9', priority: 'Medium' }),
  ])
  assert.deepEqual(ids(selection), ['A-1', 'M-9', 'Z-2'])
})

test('unmet dependencies hold a story back and are counted', () => {
  const selection = selectNextWork([
    story({ id: 'DEP-1', status: 'Complete' }),
    // DEP-2 is not Complete but also not actionable itself — the cleanest
    // fixture to prove the dependency gate without the dependency also
    // appearing as eligible work.
    story({ id: 'DEP-2', status: 'Blocked' }),
    story({ id: 'READY-1', status: 'Planned', dependencies: 'needs DEP-1' }),
    story({ id: 'WAITING', status: 'Planned', dependencies: 'needs DEP-2' }),
    story({ id: 'READY-2', status: 'Planned' }),
  ])
  assert.deepEqual(ids(selection), ['READY-1', 'READY-2'])
  assert.equal(selection.totalEligible, 2)
  assert.equal(selection.totalBlockedByDependency, 1)
})

test('references to stories the board does not know never block', () => {
  const selection = selectNextWork([
    story({ id: 'A-1', status: 'Planned', dependencies: 'needs FUTURE-X' }),
    story({ id: 'A-2', status: 'Planned' }),
  ])
  assert.deepEqual(ids(selection), ['A-1', 'A-2'])
  assert.equal(selection.totalBlockedByDependency, 0)
})

test('dependency matching is case-insensitive and Complete unblocks', () => {
  const blocked = selectNextWork([
    story({ id: 'CRM-14B', status: 'Complete' }),
    story({ id: 'A-1', status: 'Planned', dependencies: 'depends on crm-14b' }),
  ])
  assert.deepEqual(ids(blocked), ['A-1'])

  // Not Complete → A-1 is held back; the dependency story is itself
  // non-actionable (Blocked) so it cannot pollute the selection.
  const unblocked = selectNextWork([
    story({ id: 'CRM-14B', status: 'Blocked' }),
    story({ id: 'A-1', status: 'Planned', dependencies: 'depends on crm-14b' }),
  ])
  assert.deepEqual(ids(unblocked), [])
  assert.equal(unblocked.totalBlockedByDependency, 1)
})

test('the selection is deterministic and never mutates its input', () => {
  const board = [
    story({ id: 'Z-1', priority: 'High', batch: 2 }),
    story({ id: 'A-1', priority: 'Medium' }),
    story({ id: 'B-1', priority: 'High' }),
    story({ id: 'DONE', status: 'Complete' }),
  ]
  const before = board.map((s) => s.id)
  const first = selectNextWork(board)
  const second = selectNextWork(board)
  assert.deepEqual(ids(first), ids(second))
  assert.deepEqual(first, second)
  assert.deepEqual(
    board.map((s) => s.id),
    before,
  )
})

test('an empty board yields an empty selection', () => {
  const selection = selectNextWork([])
  assert.deepEqual(selection.entries, [])
  assert.equal(selection.totalEligible, 0)
  assert.equal(selection.totalBlockedByDependency, 0)
  assert.equal(selection.truncated, false)
  assert.equal(selection.limit, NEXT_WORK_DEFAULT_LIMIT)
})
