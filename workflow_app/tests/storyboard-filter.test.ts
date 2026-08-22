import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  defaultStoryBoardFilter,
  filterStories,
  isStoryBoardFilterActive,
  parseStoryBoardFilter,
  storyBoardFilterToQuery,
  storyMatchesQuery,
  type StoryRecord,
} from '../../lib/storyboard-data'

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
    completion: overrides.completion ?? 0,
    rollup: overrides.rollup ?? true,
    plannedStartAt: overrides.plannedStartAt ?? null,
    actualStartAt: overrides.actualStartAt ?? null,
    completedAt: overrides.completedAt ?? null,
    createdAt: overrides.createdAt ?? '2026-08-21T00:00:00Z',
    updatedAt: overrides.updatedAt ?? '2026-08-21T00:00:00Z',
  }
}

const sample = [
  story({
    id: 'ENG-04',
    workstream: 'HARDEN',
    title: 'TUNIT Formal Regression Suite',
    priority: 'High',
    status: 'Planned',
    notes: 'Convert expensive one-off workflow proofs into durable tests.',
    goal: 'Durable regression suite behind one canonical command.',
    architectBrief: 'Keep workflow_engine generic; reuse the in-memory fakes.',
    acceptanceCriteria: 'One canonical test command runs the whole suite.',
  }),
  story({
    id: 'CRM-14B',
    workstream: 'TXN',
    title: 'Concurrent Join Contention Proof',
    priority: 'High',
    status: 'Blocked',
    notes: 'Sequential release is proven; contention proof remains.',
    rollup: true,
  }),
  story({
    id: 'CRM-14',
    workstream: 'TXN',
    title: 'Transaction Workflow Kernel',
    priority: 'High',
    status: 'Complete',
    notes: 'Parent story; children carry rollup weight.',
    rollup: false,
  }),
  story({
    id: 'AUTH-02',
    workstream: 'AUTH',
    title: 'Portal Authorization',
    priority: 'Critical',
    status: 'Deferred',
    notes: 'Roles/permissions and protected routes.',
  }),
]

test('default filter is inactive and matches everything', () => {
  const f = defaultStoryBoardFilter()
  assert.equal(isStoryBoardFilterActive(f), false)
  assert.equal(filterStories(sample, f).length, sample.length)
})

test('search matches story ID case-insensitively', () => {
  assert.equal(storyMatchesQuery(sample[0], 'eng-04'), true)
  assert.equal(storyMatchesQuery(sample[0], 'ENG-04'), true)
})

test('search matches title', () => {
  assert.equal(storyMatchesQuery(sample[0], 'formal regression'), true)
})

test('search matches notes', () => {
  assert.equal(storyMatchesQuery(sample[1], 'sequential release'), true)
})

test('search matches architect brief', () => {
  assert.equal(storyMatchesQuery(sample[0], 'in-memory fakes'), true)
})

test('search matches acceptance criteria', () => {
  assert.equal(storyMatchesQuery(sample[0], 'canonical test command'), true)
})

test('search does not match absent text', () => {
  assert.equal(storyMatchesQuery(sample[0], 'zebra'), false)
})

test('workstream filter narrows the list', () => {
  const f = { ...defaultStoryBoardFilter(), workstream: 'TXN' }
  const result = filterStories(sample, f)
  assert.deepEqual(result.map((s) => s.id), ['CRM-14B', 'CRM-14'])
})

test('status filter works', () => {
  const f = { ...defaultStoryBoardFilter(), status: 'Blocked' }
  const result = filterStories(sample, f)
  assert.deepEqual(result.map((s) => s.id), ['CRM-14B'])
})

test('priority filter works', () => {
  const f = { ...defaultStoryBoardFilter(), priority: 'Critical' }
  const result = filterStories(sample, f)
  assert.deepEqual(result.map((s) => s.id), ['AUTH-02'])
})

test('open work view = Planned + In Progress + Partial', () => {
  const f = { ...defaultStoryBoardFilter(), view: 'open' }
  const result = filterStories(sample, f)
  assert.deepEqual(result.map((s) => s.id), ['ENG-04'])
})

test('blocked/failed view', () => {
  const f = { ...defaultStoryBoardFilter(), view: 'blocked-failed' }
  const result = filterStories(sample, f)
  assert.deepEqual(result.map((s) => s.id), ['CRM-14B'])
})

test('complete view', () => {
  const f = { ...defaultStoryBoardFilter(), view: 'complete' }
  const result = filterStories(sample, f)
  assert.deepEqual(result.map((s) => s.id), ['CRM-14'])
})

test('deferred/hold view', () => {
  const f = { ...defaultStoryBoardFilter(), view: 'deferred-hold' }
  const result = filterStories(sample, f)
  assert.deepEqual(result.map((s) => s.id), ['AUTH-02'])
})

test('rollup in/out filters', () => {
  const inRollup = filterStories(sample, {
    ...defaultStoryBoardFilter(),
    rollup: 'in',
  })
  assert.deepEqual(inRollup.map((s) => s.id), ['ENG-04', 'CRM-14B', 'AUTH-02'])
  const outRollup = filterStories(sample, {
    ...defaultStoryBoardFilter(),
    rollup: 'out',
  })
  assert.deepEqual(outRollup.map((s) => s.id), ['CRM-14'])
})

test('filters combine rather than overwrite', () => {
  const f = {
    ...defaultStoryBoardFilter(),
    workstream: 'TXN',
    view: 'open',
  }
  const result = filterStories(sample, f)
  assert.deepEqual(result.map((s) => s.id), [])
})

test('search + view combine', () => {
  const f = { ...defaultStoryBoardFilter(), q: 'concurrency', view: 'open' }
  const result = filterStories(sample, f)
  assert.deepEqual(result.map((s) => s.id), [])
})

test('parseStoryBoardFilter reads stable URL parameter names', () => {
  const f = parseStoryBoardFilter({
    q: 'concurrency',
    workstream: 'TXN',
    status: 'Blocked',
    priority: 'High',
    view: 'open',
    rollup: 'in',
  })
  assert.equal(f.q, 'concurrency')
  assert.equal(f.workstream, 'TXN')
  assert.equal(f.status, 'Blocked')
  assert.equal(f.priority, 'High')
  assert.equal(f.view, 'open')
  assert.equal(f.rollup, 'in')
})

test('parseStoryBoardFilter ignores invalid values and empty query strings', () => {
  const f = parseStoryBoardFilter({
    q: '',
    workstream: 'NOT-A-WORKSTREAM',
    status: 'Bogus',
    priority: 'Bogus',
    view: 'bogus',
    rollup: 'maybe',
  })
  assert.deepEqual(f, defaultStoryBoardFilter())
})

test('storyBoardFilterToQuery round-trips through parse', () => {
  const f: ReturnType<typeof defaultStoryBoardFilter> = {
    q: 'concurrency',
    workstream: 'TXN',
    status: 'all',
    priority: 'High',
    view: 'blocked-failed',
    rollup: 'all',
    surface: 'all',
  }
  const qs = storyBoardFilterToQuery(f)
  const parsed = parseStoryBoardFilter(
    Object.fromEntries(new URLSearchParams(qs)),
  )
  assert.deepEqual(parsed, f)
})

test('storyBoardFilterToQuery omits all defaults', () => {
  assert.equal(storyBoardFilterToQuery(defaultStoryBoardFilter()), '')
})
