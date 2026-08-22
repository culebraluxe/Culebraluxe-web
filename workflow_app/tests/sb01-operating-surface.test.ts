import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  buildStoryBoardModel,
  defaultStoryBoardFilter,
  filterStories,
  parseStoryBoardFilter,
  storyBoardFilterToQuery,
  type StoryRecord,
} from '../../lib/storyboard-data'
import type { CommandReceipt } from '../../lib/commands/contracts'

// ---------------------------------------------------------------------------
// SB-01 — Operating Surface Classification + Rollups: pure-model verification.
//
// The classification mapping itself is a deterministic SQL VALUES list in
// db/migrations/053_storyboard_sb01_operating_surface.sql; these tests prove
// the projection semantics that read the classified column:
//   * representative NEXUS / OPS / SUPPORT / TECH rollups
//   * rollup=false / reference rows never pollute the percentages
//   * Net-Net is numerically unchanged by classification
//   * workstream is untouched (surfaces are an independent axis)
//   * projections are deterministic
//   * NULL remains valid and is excluded from every surface projection
//   * the surface filter parses / serializes / filters correctly
// ---------------------------------------------------------------------------

function story(overrides: Partial<StoryRecord> & { id: string }): StoryRecord {
  return {
    id: overrides.id,
    workstream: overrides.workstream ?? 'CRM',
    operatingSurface: overrides.operatingSurface ?? null,
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

function projection(
  model: ReturnType<typeof buildStoryBoardModel>,
  surface: string,
) {
  const rollup = model.surfaceRollups.find((r) => r.surface === surface)
  assert.ok(rollup, `surface ${surface} must have a projection`)
  return rollup
}

test('representative NEXUS stories roll up by the stored completion', () => {
  const model = buildStoryBoardModel([
    story({ id: 'N-1', operatingSurface: 'NEXUS', status: 'Complete' }),
    story({ id: 'N-2', operatingSurface: 'NEXUS', status: 'Partial', completion: 50 }),
    story({ id: 'N-3', operatingSurface: 'NEXUS', status: 'Planned', completion: 0 }),
  ])
  // AVG(100, 50, 0) = 50.
  const nexus = projection(model, 'NEXUS')
  assert.equal(nexus.completionPercent, 50)
  assert.equal(nexus.storyCount, 3)
  assert.equal(nexus.completeCount, 1)
  assert.equal(nexus.openCount, 1)
})

test('representative OPS stories roll up by the stored completion', () => {
  const model = buildStoryBoardModel([
    story({ id: 'O-1', operatingSurface: 'OPS', status: 'Complete' }),
    story({ id: 'O-2', operatingSurface: 'OPS', status: 'Complete' }),
  ])
  assert.equal(projection(model, 'OPS').completionPercent, 100)
})
// __PART2__
test('representative SUPPORT stories roll up by the stored completion', () => {
  const model = buildStoryBoardModel([
    story({ id: 'S-1', operatingSurface: 'SUPPORT', status: 'Complete' }),
    story({ id: 'S-2', operatingSurface: 'SUPPORT', status: 'Partial', completion: 20 }),
  ])
  // AVG(100, 20) = 60.
  assert.equal(projection(model, 'SUPPORT').completionPercent, 60)
})

test('representative TECH stories roll up by the stored completion', () => {
  const model = buildStoryBoardModel([
    story({ id: 'T-1', operatingSurface: 'TECH', status: 'Complete' }),
    story({ id: 'T-2', operatingSurface: 'TECH', status: 'Planned', completion: 40 }),
  ])
  // AVG(100, 40) = 70.
  assert.equal(projection(model, 'TECH').completionPercent, 70)
})

test('rollup=false / reference rows never pollute the surface percentages', () => {
  const model = buildStoryBoardModel([
    // Two reference rows (ARCH-HANDOFF keeps NULL; SOP1 is SUPPORT).
    story({ id: 'ARCH-HANDOFF', operatingSurface: null, rollup: false, status: 'Complete' }),
    story({ id: 'SOP1', operatingSurface: 'SUPPORT', rollup: false, status: 'Complete' }),
    // One real rollup participant per surface.
    story({ id: 'N-1', operatingSurface: 'NEXUS', status: 'Complete' }),
    story({ id: 'O-1', operatingSurface: 'OPS', status: 'Complete' }),
    story({ id: 'S-1', operatingSurface: 'SUPPORT', status: 'Complete' }),
    story({ id: 'T-1', operatingSurface: 'TECH', status: 'Complete' }),
  ])
  const support = projection(model, 'SUPPORT')
  // SOP1 (rollup=false, completion 100) must NOT inflate the percentage.
  assert.equal(support.storyCount, 1)
  assert.equal(support.completionPercent, 100)
  assert.equal(support.storedCount, 2, 'reference rows still appear in storedCount')
})

test('NULL operating_surface stories are excluded and counted as unclassified', () => {
  const model = buildStoryBoardModel([
    story({ id: 'ARCH-HANDOFF', operatingSurface: null, rollup: false }),
    story({ id: 'N-1', operatingSurface: 'NEXUS', status: 'Complete' }),
  ])
  assert.equal(model.unclassifiedCount, 1)
  assert.equal(projection(model, 'NEXUS').storyCount, 1)
  // The unclassified row must not appear in any surface projection.
  for (const rollup of model.surfaceRollups) {
    assert.equal(rollup.storyCount, rollup.surface === 'NEXUS' ? 1 : 0)
  }
})
// __PART3__
test('Net-Net is numerically unchanged by surface classification', () => {
  const baseStories = [
    story({ id: 'A-1', workstream: 'CRM', status: 'Complete' }),
    story({ id: 'A-2', workstream: 'CRM', status: 'Partial', completion: 50 }),
    story({ id: 'B-1', workstream: 'TXN', status: 'Complete' }),
  ]
  const before = buildStoryBoardModel(baseStories)
  const classified = buildStoryBoardModel([
    { ...baseStories[0], operatingSurface: 'NEXUS' },
    { ...baseStories[1], operatingSurface: 'OPS' },
    { ...baseStories[2], operatingSurface: 'TECH' },
  ])
  assert.equal(classified.netNet, before.netNet)
  assert.deepEqual(
    classified.workstreams.map((w) => w.completionPercent),
    before.workstreams.map((w) => w.completionPercent),
  )
})

test('workstream is untouched by classification (independent axis)', () => {
  const model = buildStoryBoardModel([
    story({ id: 'X-1', workstream: 'CRM', operatingSurface: 'NEXUS' }),
    story({ id: 'X-2', workstream: 'AUTH', operatingSurface: 'SUPPORT' }),
    story({ id: 'X-3', workstream: 'HARDEN', operatingSurface: 'TECH' }),
  ])
  assert.deepEqual(
    model.stories.map((s) => [s.id, s.workstream, s.operatingSurface]),
    [
      ['X-1', 'CRM', 'NEXUS'],
      ['X-2', 'AUTH', 'SUPPORT'],
      ['X-3', 'HARDEN', 'TECH'],
    ],
  )
})

test('surface projections are deterministic for identical input', () => {
  const stories = [
    story({ id: 'N-1', operatingSurface: 'NEXUS', status: 'Complete' }),
    story({ id: 'O-1', operatingSurface: 'OPS', status: 'Partial', completion: 30 }),
    story({ id: 'T-1', operatingSurface: 'TECH', status: 'Planned' }),
  ]
  const first = buildStoryBoardModel(stories)
  const second = buildStoryBoardModel(stories)
  assert.deepEqual(first.surfaceRollups, second.surfaceRollups)
})

test('surface filter parses, serializes and filters deterministically', () => {
  const stories = [
    story({ id: 'N-1', operatingSurface: 'NEXUS' }),
    story({ id: 'O-1', operatingSurface: 'OPS' }),
    story({ id: 'U-1', operatingSurface: null }),
  ]

  const nexus = parseStoryBoardFilter({ surface: 'NEXUS' })
  assert.deepEqual(
    filterStories(stories, nexus).map((s) => s.id),
    ['N-1'],
  )
  assert.equal(storyBoardFilterToQuery(nexus), 'surface=NEXUS')

  const unclassified = parseStoryBoardFilter({ surface: 'unclassified' })
  assert.deepEqual(
    filterStories(stories, unclassified).map((s) => s.id),
    ['U-1'],
  )

  const all = parseStoryBoardFilter({})
  assert.deepEqual(filterStories(stories, all).length, 3)
  assert.equal(defaultStoryBoardFilter().surface, 'all')
  assert.equal(storyBoardFilterToQuery(defaultStoryBoardFilter()), '')
})

test('unknown surface params are ignored (no invented certainty)', () => {
  const filter = parseStoryBoardFilter({ surface: 'NOPE' })
  assert.equal(filter.surface, 'all')
})

// ---------------------------------------------------------------------------
// Known-main-type-error: the canonical CommandReceipt contract accepts the
// actor that receipt-backed handlers already record. This compiles only when
// contracts.ts exposes actorAppUserId (the additive repair).
// ---------------------------------------------------------------------------
test('CommandReceipt contract accepts actorAppUserId (compile-time repair proof)', () => {
  const receipt: CommandReceipt = {
    commandId: 'cmd-1',
    outcome: 'success',
    status: 'Succeeded',
    aggregateId: 'agg-1',
    message: null,
    createdAt: '2026-08-22T00:00:00.000Z',
    actorAppUserId: 'app-user-1',
  }
  assert.equal(receipt.actorAppUserId, 'app-user-1')
})


