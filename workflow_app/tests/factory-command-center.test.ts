import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  buildFactoryCapacity,
  buildFactoryPipeline,
  extractDependencyRefs,
} from '../../lib/factory-command-center-data'
import type { StoryboardStory, StoryRun } from '../../db/storyboard'
import type { AgentWorkItem } from '../../db/agent-work'

// ENG-16 — pure unit tests for the factory command center read projections.
// In-memory fixtures only; no database, no engine.

function story(overrides: Partial<StoryboardStory> & { id: string }): StoryboardStory {
  return {
    id: overrides.id,
    workstream: overrides.workstream ?? 'HARDEN',
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

function work(overrides: Partial<AgentWorkItem> & { id: string; storyId: string }): AgentWorkItem {
  return {
    state: 'Ready',
    priority: 0,
    queuedAt: '2026-08-21T00:00:00Z',
    claimedAt: null,
    claimedBy: null,
    startedAt: null,
    finishedAt: null,
    storyRunId: null,
    errorText: null,
    role: null,
    modelProfile: null,
    specialInstructions: null,
    runtimeAdapter: null,
    externalRunId: null,
    attempts: 0,
    maxAttempts: 3,
    executionPolicy: 'Unattended OK',
    executionEnvironment: 'DEV',
    createdAt: '2026-08-21T00:00:00Z',
    updatedAt: '2026-08-21T00:00:00Z',
    ...overrides,
  }
}

function run(overrides: Partial<StoryRun> & { id: string; storyId: string }): StoryRun {
  return {
    startedAt: '2026-08-21T00:00:00Z',
    endedAt: null,
    resultStatus: null,
    completion: null,
    notes: null,
    commitHash: null,
    testsSummary: null,
    executionEnvironment: null,
    goalSnapshot: null,
    preconditionsSnapshot: null,
    architectBriefSnapshot: null,
    contextRefsSnapshot: null,
    acceptanceCriteriaSnapshot: null,
    postconditionsSnapshot: null,
    createdAt: '2026-08-21T00:00:00Z',
    updatedAt: '2026-08-21T00:00:00Z',
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// Dependency reference parsing
// ---------------------------------------------------------------------------

test('extractDependencyRefs parses comma/and prose', () => {
  assert.deepEqual(
    extractDependencyRefs('S-018, S-019 and CRM-14A'),
    ['S-018', 'S-019', 'CRM-14A'],
  )
})

test('extractDependencyRefs parses slash lists and parentheticals', () => {
  assert.deepEqual(extractDependencyRefs('S-018/S-019 (S-020)'), ['S-018', 'S-019', 'S-020'])
})

test('extractDependencyRefs normalizes case and dedupes', () => {
  assert.deepEqual(extractDependencyRefs('eng-15, ENG-15, eng-15'), ['ENG-15'])
})

test('extractDependencyRefs does not invent range members', () => {
  // "through" ranges degrade to the tokens present; no expansion is invented.
  assert.deepEqual(extractDependencyRefs('CRM-14A through CRM-14I'), ['CRM-14A', 'CRM-14I'])
})

test('extractDependencyRefs returns empty for null/blank/no-ref text', () => {
  assert.deepEqual(extractDependencyRefs(null), [])
  assert.deepEqual(extractDependencyRefs(''), [])
  assert.deepEqual(extractDependencyRefs('depends on nothing here'), [])
})

test('extractDependencyRefs ignores numeric-only and malformed tokens', () => {
  assert.deepEqual(extractDependencyRefs('story 116, 117, 135'), [])
  assert.deepEqual(extractDependencyRefs('S--018, ENG--15'), [])
})

// ---------------------------------------------------------------------------
// Pipeline derivation
// ---------------------------------------------------------------------------

test('Ready story with all dependencies Complete is eligible next', () => {
  const deps = [
    story({ id: 'ENG-15', status: 'Complete', completion: 100 }),
    story({ id: 'S-018', status: 'Complete', completion: 100 }),
  ]
  const target = story({
    id: 'ENG-16',
    status: 'Ready',
    dependencies: 'ENG-15, S-018',
  })
  const pipeline = buildFactoryPipeline([...deps, target], [], new Map())
  const node = pipeline.nodes.find((n) => n.storyId === 'ENG-16')!
  assert.equal(node.stage, 'ready')
  assert.equal(node.ready, true)
  assert.equal(node.blocked, false)
  assert.equal(node.waitingOnDeps, false)
  assert.equal(node.blockedReason, null)
  assert.deepEqual(pipeline.readyWork, ['ENG-16'])
  assert.ok(!pipeline.blockedWork.includes('ENG-16'))
})

test('Ready story waiting on an uncompleted dependency is blocked with a reason', () => {
  const dep = story({ id: 'ENG-15', status: 'In Progress', completion: 45 })
  const target = story({
    id: 'ENG-16',
    status: 'Ready',
    dependencies: 'ENG-15',
  })
  const pipeline = buildFactoryPipeline([dep, target], [], new Map())
  const node = pipeline.nodes.find((n) => n.storyId === 'ENG-16')!
  assert.equal(node.stage, 'blocked')
  assert.equal(node.waitingOnDeps, true)
  assert.equal(node.ready, false)
  assert.match(node.blockedReason ?? '', /waiting on ENG-15 \(In Progress\)/)
  assert.deepEqual(node.blockedBy, [{ storyId: 'ENG-15', status: 'In Progress', completion: 45 }])
  assert.deepEqual(pipeline.readyWork, [])
  assert.ok(pipeline.blockedWork.includes('ENG-16'))
})

test('Blocked / Failed stories are blocked regardless of dependencies', () => {
  const target = story({ id: 'CRM-11', status: 'Blocked', dependencies: 'S-002' })
  const pipeline = buildFactoryPipeline([target], [], new Map())
  const node = pipeline.nodes.find((n) => n.storyId === 'CRM-11')!
  assert.equal(node.stage, 'blocked')
  assert.equal(node.blocked, true)
  assert.match(node.blockedReason ?? '', /story status is Blocked/)
  assert.ok(pipeline.blockedWork.includes('CRM-11'))
})

test('work item Error / Cancelled blocks the story with a concise reason', () => {
  const items = [
    work({ id: 'w1', storyId: 'ENG-08', state: 'Error', claimedBy: 'builder-flash', errorText: 'stale: no heartbeat' }),
  ]
  const pipeline = buildFactoryPipeline([story({ id: 'ENG-08', status: 'Failed' })], items, new Map())
  const node = pipeline.nodes.find((n) => n.storyId === 'ENG-08')!
  assert.equal(node.blocked, true)
  assert.match(node.blockedReason ?? '', /work item failed: stale: no heartbeat/)
})

test('Planned stories keep the planned lane even with unmet dependencies', () => {
  const deps = [
    story({ id: 'S-020', status: 'Planned' }),
    story({ id: 'S-022', status: 'Planned' }),
    story({ id: 'S-026', status: 'Planned' }),
  ]
  const target = story({ id: 'S-031', status: 'Planned', dependencies: 'S-020, S-022, S-026' })
  const pipeline = buildFactoryPipeline([...deps, target], [], new Map())
  const node = pipeline.nodes.find((n) => n.storyId === 'S-031')!
  assert.equal(node.stage, 'planned')
  assert.equal(node.ready, false)
  assert.ok(!pipeline.blockedWork.includes('S-031'))
  assert.equal(node.blockedBy.length, 3)
})

test('human execution gates are derived from the latest work item policy', () => {
  const items = [
    work({ id: 'w1', storyId: 'ENG-16', state: 'Ready', executionPolicy: 'Human Gate' }),
  ]
  const pipeline = buildFactoryPipeline(
    [story({ id: 'ENG-16', status: 'Ready' })],
    items,
    new Map(),
  )
  const node = pipeline.nodes.find((n) => n.storyId === 'ENG-16')!
  assert.equal(node.gated, true)
  assert.equal(node.gate, 'Human Gate')
  assert.deepEqual(pipeline.gatedWork, ['ENG-16'])

  for (const policy of ['Manual Only', 'Daytime Only'] as const) {
    const p = buildFactoryPipeline(
      [story({ id: 'ENG-16', status: 'Ready' })],
      [work({ id: 'w2', storyId: 'ENG-16', state: 'Ready', executionPolicy: policy })],
      new Map(),
    )
    const n = p.nodes.find((x) => x.storyId === 'ENG-16')!
    assert.equal(n.gated, true)
    assert.equal(n.gate, policy)
  }
})

test('node carries the assigned worker and concise evidence state', () => {
  const items = [
    work({
      id: 'w1',
      storyId: 'ENG-16',
      state: 'Running',
      claimedBy: 'deepseek-runtime',
      role: 'builder',
    }),
  ]
  const runs = new Map<string, StoryRun>([
    [
      'ENG-16',
      run({
        id: 'r1',
        storyId: 'ENG-16',
        resultStatus: 'Partial',
        completion: 60,
        testsSummary: 'targeted 12/12 passed',
        commitHash: 'abcdef123456',
        notes: '2026-08-22 10:00 — inspected seam\n2026-08-22 10:30 — implemented read model',
      }),
    ],
  ])
  const pipeline = buildFactoryPipeline(
    [story({ id: 'ENG-16', status: 'In Progress', completion: 60 })],
    items,
    runs,
  )
  const node = pipeline.nodes.find((n) => n.storyId === 'ENG-16')!
  assert.equal(node.worker, 'deepseek-runtime')
  assert.equal(node.role, 'builder')
  assert.equal(node.workState, 'Running')
  assert.equal(node.runResult, 'Partial')
  assert.equal(node.runCompletion, 60)
  assert.equal(node.testsSummary, 'targeted 12/12 passed')
  assert.equal(node.commitHash, 'abcdef123456')
  // latestStep is the LAST narrative line, not invented.
  assert.match(node.latestStep ?? '', /implemented read model/)
})

test('dependency edges explain satisfaction and external refs never block', () => {
  const dep = story({ id: 'ENG-15', status: 'Complete' })
  const inProgress = story({ id: 'S-018', status: 'In Progress' })
  const target = story({
    id: 'ENG-16',
    status: 'Ready',
    dependencies: 'ENG-15, S-018, UNKNOWN-99',
  })
  const pipeline = buildFactoryPipeline([dep, inProgress, target], [], new Map())
  const edges = pipeline.edges.filter((e) => e.from === 'ENG-16')
  assert.equal(edges.length, 3)

  const satisfied = edges.find((e) => e.to === 'ENG-15')!
  assert.equal(satisfied.external, false)
  assert.equal(satisfied.satisfied, true)
  assert.equal(satisfied.toStatus, 'Complete')

  const waiting = edges.find((e) => e.to === 'S-018')!
  assert.equal(waiting.satisfied, false)
  assert.equal(waiting.toStatus, 'In Progress')

  const external = edges.find((e) => e.to === null)!
  assert.equal(external.external, true)
  assert.equal(external.satisfied, null)
  assert.equal(external.toStatus, null)
  // The external ref did not block eligibility; the known unmet one did.
  assert.deepEqual(
    pipeline.nodes.find((n) => n.storyId === 'ENG-16')!.blockedBy,
    [{ storyId: 'S-018', status: 'In Progress', completion: 0 }],
  )
})

// ---------------------------------------------------------------------------
// Agent dispatch / capacity
// ---------------------------------------------------------------------------

test('factory is available with capacity when no command is in flight', () => {
  const stories = [story({ id: 'ENG-16', status: 'Ready' })]
  const pipeline = buildFactoryPipeline(stories, [], new Map())
  const capacity = buildFactoryCapacity(stories, [], pipeline)
  assert.equal(capacity.busyCount, 0)
  assert.equal(capacity.blockedCount, 0)
  assert.equal(capacity.availableCount, 1)
  assert.equal(capacity.workers[0].kind, 'available')
  assert.deepEqual(capacity.nextEligible.map((s) => s.storyId), ['ENG-16'])
})

test('busy worker is derived from the active command', () => {
  const stories = [story({ id: 'ENG-16', status: 'In Progress' })]
  const items = [
    work({
      id: 'w1',
      storyId: 'ENG-16',
      state: 'Running',
      claimedBy: 'deepseek-runtime',
      role: 'builder',
      claimedAt: '2026-08-22T10:00:00Z',
    }),
  ]
  const pipeline = buildFactoryPipeline(stories, items, new Map())
  const capacity = buildFactoryCapacity(stories, items, pipeline)
  assert.equal(capacity.busyCount, 1)
  const worker = capacity.workers.find((w) => w.kind === 'busy')!
  assert.equal(worker.workerId, 'deepseek-runtime')
  assert.equal(worker.storyId, 'ENG-16')
  assert.equal(worker.workState, 'Running')
  assert.equal(worker.since, '2026-08-22T10:00:00Z')
  assert.equal(capacity.availableCount, 0)
})

test('a paused command holds a waiting slot (not busy, not available)', () => {
  const stories = [story({ id: 'ENG-16', status: 'In Progress' })]
  const items = [
    work({
      id: 'w1',
      storyId: 'ENG-16',
      state: 'Paused',
      claimedBy: 'deepseek-runtime',
      claimedAt: '2026-08-22T10:00:00Z',
    }),
  ]
  const pipeline = buildFactoryPipeline(stories, items, new Map())
  const capacity = buildFactoryCapacity(stories, items, pipeline)
  assert.equal(capacity.waitingCount, 1)
  assert.equal(capacity.busyCount, 0)
  assert.equal(capacity.availableCount, 0)
  const worker = capacity.workers.find((w) => w.kind === 'waiting')!
  assert.equal(worker.storyId, 'ENG-16')
  assert.equal(worker.workState, 'Paused')
})

test('a failed command blocks a story but releases the execution slot', () => {
  const stories = [story({ id: 'ENG-08', status: 'Failed' })]
  const items = [
    work({
      id: 'w1',
      storyId: 'ENG-08',
      state: 'Error',
      claimedBy: 'builder-flash',
      finishedAt: '2026-08-22T11:00:00Z',
    }),
  ]
  const pipeline = buildFactoryPipeline(stories, items, new Map())
  const capacity = buildFactoryCapacity(stories, items, pipeline)
  assert.equal(capacity.blockedCount, 1)
  // One failed story costs one story, not the shift: the slot is free again.
  assert.equal(capacity.availableCount, 1)
  const worker = capacity.workers.find((w) => w.kind === 'blocked')!
  assert.equal(worker.workerId, 'builder-flash')
  assert.equal(worker.storyId, 'ENG-08')
  assert.equal(worker.workState, 'Error')
})

test('next eligible is priority-ordered and limited to dependency-ready work', () => {
  const stories = [
    story({ id: 'LOW-1', status: 'Ready', priority: 'Low' }),
    story({ id: 'CRIT-1', status: 'Ready', priority: 'Critical' }),
    story({ id: 'ENG-15', status: 'In Progress' }),
    story({ id: 'WAIT-1', status: 'Ready', priority: 'High', dependencies: 'ENG-15' }),
    story({ id: 'HIGH-1', status: 'Ready', priority: 'High' }),
    story({ id: 'DONE-1', status: 'Complete', priority: 'High' }),
  ]
  const pipeline = buildFactoryPipeline(stories, [], new Map())
  const capacity = buildFactoryCapacity(stories, [], pipeline)
  assert.deepEqual(
    capacity.nextEligible.map((s) => s.storyId),
    ['CRIT-1', 'HIGH-1', 'LOW-1'],
  )
})

test('latest work item wins when a story has multiple commands', () => {
  const stories = [story({ id: 'ENG-16', status: 'In Progress' })]
  const items = [
    work({
      id: 'w1',
      storyId: 'ENG-16',
      state: 'Done',
      claimedBy: 'builder-flash',
      queuedAt: '2026-08-21T00:00:00Z',
    }),
    work({
      id: 'w2',
      storyId: 'ENG-16',
      state: 'Running',
      claimedBy: 'deepseek-runtime',
      queuedAt: '2026-08-22T00:00:00Z',
    }),
  ]
  const pipeline = buildFactoryPipeline(stories, items, new Map())
  const node = pipeline.nodes.find((n) => n.storyId === 'ENG-16')!
  assert.equal(node.worker, 'deepseek-runtime')
  assert.equal(node.workState, 'Running')
})
