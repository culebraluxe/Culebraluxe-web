import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  buildFactoryKpis,
  assessFactoryHealth,
  recommendNextDispatch,
  priorityRankOf,
  type FactoryKpiInput,
} from '../../lib/factory-kpi'
import { buildStoryBoardModel } from '../../lib/storyboard-data'
import { buildFactoryPipeline, buildFactoryCapacity } from '../../lib/factory-command-center-data'
import type { StoryboardStory, StoryRun } from '../../db/storyboard'
import type { AgentWorkItem } from '../../db/agent-work'

// ENG-17 — pure unit tests for the factory KPI + dispatch model.
// In-memory fixtures only; no database, no engine. `now` is always injected so
// every assertion is deterministic.

const NOW = '2026-08-22T12:00:00.000Z'

function story(overrides: Partial<StoryboardStory> & { id: string }): StoryboardStory {
  return {
    id: overrides.id,
    workstream: overrides.workstream ?? 'HARDEN',
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
    completion: overrides.completion ?? 0,
    rollup: overrides.rollup ?? true,
    plannedStartAt: overrides.plannedStartAt ?? null,
    actualStartAt: overrides.actualStartAt ?? null,
    completedAt: overrides.completedAt ?? null,
    createdAt: overrides.createdAt ?? '2026-08-01T00:00:00Z',
    updatedAt: overrides.updatedAt ?? '2026-08-22T00:00:00Z',
  }
}

function work(overrides: Partial<AgentWorkItem> & { id: string; storyId: string }): AgentWorkItem {
  return {
    state: 'Ready',
    priority: 0,
    queuedAt: '2026-08-22T08:00:00Z',
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
    createdAt: '2026-08-22T08:00:00Z',
    updatedAt: '2026-08-22T08:00:00Z',
    ...overrides,
  }
}

function run(overrides: Partial<StoryRun> & { id: string; storyId: string }): StoryRun {
  return {
    startedAt: '2026-08-21T09:00:00Z',
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
    createdAt: '2026-08-21T09:00:00Z',
    updatedAt: '2026-08-21T09:00:00Z',
    ...overrides,
  }
}

/** Build the full KPI input the way the snapshot loader does: real pipeline +
 *  real capacity + real rollup, all pure and in-memory. */
function kpiInput(
  stories: StoryboardStory[],
  workItems: AgentWorkItem[],
  runs: StoryRun[],
  overrides: Partial<FactoryKpiInput> = {},
): FactoryKpiInput {
  const pipeline = buildFactoryPipeline(stories, workItems, new Map())
  const capacity = buildFactoryCapacity(stories, workItems, pipeline)
  return {
    stories,
    workItems,
    runs,
    pipeline: {
      readyWork: pipeline.readyWork,
      gatedWork: pipeline.gatedWork,
      blockedWork: pipeline.blockedWork,
      nodes: pipeline.nodes.map((n) => ({
        storyId: n.storyId,
        blockedBy: n.blockedBy,
        gated: n.gated,
        gate: n.gate,
      })),
    },
    slots: capacity.workers.map((w) => ({
      kind: w.kind,
      workerId: w.workerId,
      role: w.role,
      modelProfile: w.modelProfile,
      storyId: w.storyId,
      workState: w.workState,
      since: w.since,
    })),
    rollup: buildStoryBoardModel(stories),
    nowIso: NOW,
    ...overrides,
  }
}

function kpiValue(kpis: { value: number | null }[], id: string): number | null {
  return kpis.find((k) => 'id' in k && (k as { id: string }).id === id)?.value ?? null
}

// ---------------------------------------------------------------------------
// Outcome KPIs
// ---------------------------------------------------------------------------

test('outcome: net-net and completion rate come from the rollup model', () => {
  const stories = [
    story({ id: 'ENG-15', status: 'Complete', completion: 100, operatingSurface: 'TECH' }),
    story({ id: 'ENG-16', status: 'In Progress', completion: 60, operatingSurface: 'TECH' }),
    story({ id: 'ENG-17', status: 'Planned', completion: 0, operatingSurface: 'TECH' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, [], []))
  // Net-net comes from the rollup model's CURRENT-SCOPE completion. ENG-17
  // (Planned / Backlog) is excluded from both terms: TECH = (100+60)/2 = 80.
  assert.equal(kpiValue(kpis.outcome, 'net-net completion'), 80)
  // Completion rate is a status-based ratio over ALL rollup stories (1 of 3 Complete).
  assert.equal(kpiValue(kpis.outcome, 'completion rate'), 33.3)
})

test('outcome: parent/non-rollup stories are excluded from completion rate', () => {
  // ENG-20-SMOKE-001 is a parent (rollup=false) — its completion must not
  // dilute or double-count the rollup completion rate.
  const stories = [
    story({ id: 'ENG-16', status: 'Complete', completion: 100 }),
    story({ id: 'ENG-17', status: 'Complete', completion: 100 }),
    story({ id: 'ENG-20-SMOKE-001', status: 'Complete', completion: 100, rollup: false }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, [], []))
  assert.equal(kpiValue(kpis.outcome, 'completion rate'), 100)
  assert.equal(kpis.scope.rollupStoryCount, 2)
  assert.equal(kpis.scope.parentStoryCount, 1)
  assert.deepEqual(kpis.scope.parentStoryIds, ['ENG-20-SMOKE-001'])
})

test('outcome: throughput counts only completions inside the window', () => {
  const stories = [
    story({
      id: 'ENG-14',
      status: 'Complete',
      completedAt: '2026-08-22T10:00:00Z', // in window
      actualStartAt: '2026-08-22T08:00:00Z',
    }),
    story({
      id: 'ENG-13',
      status: 'Complete',
      completedAt: '2026-07-01T10:00:00Z', // outside 30d window
      actualStartAt: '2026-07-01T08:00:00Z',
    }),
    story({ id: 'ENG-15', status: 'In Progress' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, [], []))
  assert.equal(kpiValue(kpis.outcome, 'throughput (window)'), 1)
  // Cycle time for the one in-window completion: 2 hours.
  assert.equal(kpiValue(kpis.outcome, 'cycle time'), 2)
})

test('outcome: cycle time is null with a missing-telemetry reason when actual_start_at is absent', () => {
  const stories = [
    story({
      id: 'ENG-14',
      status: 'Complete',
      completedAt: '2026-08-22T10:00:00Z',
      actualStartAt: null, // no run telemetry on legacy completions
    }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, [], []))
  const cycle = kpis.outcome.find((k) => k.id === 'cycle time')!
  assert.equal(cycle.value, null)
  assert.match(cycle.missingReason ?? '', /actual_start_at is null/)
})

test('outcome: run pass/fail rates use terminal runs in the window', () => {
  const runs = [
    run({ id: 'r1', storyId: 'ENG-14', resultStatus: 'Complete', startedAt: '2026-08-21T10:00:00Z' }),
    run({ id: 'r2', storyId: 'ENG-15', resultStatus: 'Complete', startedAt: '2026-08-21T11:00:00Z' }),
    run({ id: 'r3', storyId: 'ENG-16', resultStatus: 'Failed', startedAt: '2026-08-21T12:00:00Z' }),
    run({ id: 'r4', storyId: 'ENG-17', resultStatus: 'Failed', startedAt: '2026-08-21T13:00:00Z' }),
    run({ id: 'r5', storyId: 'ENG-18', resultStatus: null, startedAt: '2026-08-22T09:00:00Z' }), // live, not terminal
    run({ id: 'r6', storyId: 'ENG-01', resultStatus: 'Complete', startedAt: '2026-06-01T00:00:00Z' }), // outside window
  ]
  const kpis = buildFactoryKpis(kpiInput([], [], runs))
  assert.equal(kpiValue(kpis.outcome, 'run pass rate'), 50)
  assert.equal(kpiValue(kpis.outcome, 'run failure rate'), 50)
})

test('outcome: retry rate counts claimed commands with attempts >= 2', () => {
  const items = [
    work({ id: 'w1', storyId: 'ENG-16', attempts: 3 }), // retried
    work({ id: 'w2', storyId: 'ENG-17', attempts: 1 }), // claimed once
    work({ id: 'w3', storyId: 'ENG-18', attempts: 0 }), // never claimed
  ]
  const kpis = buildFactoryKpis(kpiInput([], items, []))
  assert.equal(kpiValue(kpis.outcome, 'retry rate'), 50)
})

// ---------------------------------------------------------------------------
// Flow KPIs
// ---------------------------------------------------------------------------

test('flow: WIP counts only rollup stories in progress or partial', () => {
  const stories = [
    story({ id: 'ENG-16', status: 'In Progress', completion: 40 }),
    story({ id: 'ENG-17', status: 'Partial', completion: 60 }),
    story({ id: 'ENG-18', status: 'Ready' }),
    story({ id: 'ENG-19', status: 'Complete' }),
    story({ id: 'ENG-20-SMOKE-001', status: 'In Progress', completion: 50, rollup: false }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, [], []))
  assert.equal(kpiValue(kpis.flow, 'wip'), 2)
})

test('flow: ready count respects dependencies — eligible vs waiting on deps', () => {
  const stories = [
    story({ id: 'ENG-15', status: 'Complete' }),
    story({ id: 'ENG-16', status: 'Ready', dependencies: 'ENG-15' }), // eligible
    story({ id: 'ENG-17', status: 'In Progress' }),
    story({ id: 'ENG-18', status: 'Ready', dependencies: 'ENG-17' }), // waiting on deps
    story({ id: 'ENG-19', status: 'Ready' }), // eligible
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, [], []))
  assert.equal(kpiValue(kpis.flow, 'ready (authorized)'), 3)
  assert.equal(kpiValue(kpis.flow, 'ready eligible'), 2)
  assert.equal(kpiValue(kpis.flow, 'ready waiting on deps'), 1)
})

test('flow: queue age uses the Ready work item queued_at', () => {
  const stories = [
    story({ id: 'ENG-19', status: 'Ready', updatedAt: '2026-08-22T11:30:00Z' }),
    story({ id: 'ENG-20', status: 'Ready' }),
  ]
  const items = [
    work({ id: 'w1', storyId: 'ENG-20', state: 'Ready', queuedAt: '2026-08-22T04:00:00Z' }), // 8h old
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  // Oldest eligible = ENG-20 with the work-item queued_at (8h), not the story
  // updated_at fallback.
  assert.equal(kpiValue(kpis.flow, 'queue age (oldest eligible)'), 8)
})

test('flow: blocked age and stale active detection', () => {
  const stories = [
    story({ id: 'ENG-08', status: 'Failed', updatedAt: '2026-08-22T06:00:00Z' }), // 6h blocked
  ]
  const items = [
    work({ id: 'w1', storyId: 'ENG-99', state: 'Running', updatedAt: '2026-08-22T11:59:00Z' }), // fresh
    work({ id: 'w2', storyId: 'ENG-98', state: 'Claimed', updatedAt: '2026-08-22T09:00:00Z' }), // stale > 60m
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpiValue(kpis.flow, 'blocked work'), 1)
  assert.equal(kpiValue(kpis.flow, 'blocked age (max)'), 6)
  assert.equal(kpiValue(kpis.flow, 'stale active'), 1)
})

// ---------------------------------------------------------------------------
// Capacity KPIs — four states
// ---------------------------------------------------------------------------

test('capacity: busy/waiting/blocked/available are distinguished', () => {
  const stories = [
    story({ id: 'ENG-16', status: 'In Progress' }),
    story({ id: 'ENG-08', status: 'Failed' }),
  ]
  const items = [
    work({ id: 'w1', storyId: 'ENG-16', state: 'Running', claimedBy: 'deepseek-runtime', role: 'builder', modelProfile: 'deepseek-v4', claimedAt: '2026-08-22T10:00:00Z' }),
    work({ id: 'w2', storyId: 'ENG-08', state: 'Error', claimedBy: 'builder-flash', role: 'builder', modelProfile: 'flash', finishedAt: '2026-08-22T09:00:00Z' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.capacity.busyCount, 1)
  assert.equal(kpis.capacity.blockedCount, 1)
  assert.equal(kpis.capacity.availableCount, 0) // slot busy
  assert.equal(kpis.capacity.waitingCount, 0)
  const busy = kpis.capacity.byCapability.find((c) => c.kind === 'busy')!
  assert.equal(busy.capability, 'builder/deepseek-v4')
})

test('capacity: paused command is waiting, not busy; slot not available', () => {
  const stories = [story({ id: 'ENG-16', status: 'In Progress' })]
  const items = [
    work({ id: 'w1', storyId: 'ENG-16', state: 'Paused', claimedBy: 'deepseek-runtime', role: 'builder', modelProfile: 'deepseek-v4' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.capacity.waitingCount, 1)
  assert.equal(kpis.capacity.busyCount, 0)
  assert.equal(kpis.capacity.availableCount, 0)
})

test('capacity: terminal failure releases the slot — available alongside blocked', () => {
  const stories = [story({ id: 'ENG-08', status: 'Failed' })]
  const items = [
    work({ id: 'w1', storyId: 'ENG-08', state: 'Error', claimedBy: 'builder-flash', finishedAt: '2026-08-22T09:00:00Z' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.capacity.blockedCount, 1)
  assert.equal(kpis.capacity.availableCount, 1) // one failed story, not a dead shift
})

test('capacity: capability demand is derived from eligible commands', () => {
  const stories = [story({ id: 'ENG-19', status: 'Ready' })]
  const items = [
    work({ id: 'w1', storyId: 'ENG-19', state: 'Ready', role: 'builder', modelProfile: 'deepseek-v4' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.deepEqual(kpis.capacity.demandByCapability, [
    { capability: 'builder/deepseek-v4', role: 'builder', modelProfile: 'deepseek-v4', count: 1 },
  ])
})

// ---------------------------------------------------------------------------
// Decision signals
// ---------------------------------------------------------------------------

test('decision: auto-dispatch eligible excludes human-gated work', () => {
  const stories = [
    story({ id: 'ENG-19', status: 'Ready' }),
    story({ id: 'ENG-20', status: 'Ready' }),
  ]
  const items = [
    work({ id: 'w1', storyId: 'ENG-20', state: 'Ready', executionPolicy: 'Human Gate' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.decision.readyEligibleCount, 2)
  assert.equal(kpis.decision.autoDispatchEligibleCount, 1)
  assert.deepEqual(kpis.decision.autoDispatchEligible, ['ENG-19'])
  assert.equal(kpis.decision.humanGateCount, 1)
})

test('decision: critical dependency pressure flags wedged dependencies', () => {
  const stories = [
    story({ id: 'ENG-10', status: 'Blocked' }),
    story({ id: 'ENG-11', status: 'Failed' }),
    story({ id: 'ENG-12', status: 'Complete' }),
    story({ id: 'ENG-13', status: 'Planned', dependencies: 'ENG-10, ENG-12' }), // wedge on ENG-10
    story({ id: 'ENG-14', status: 'Planned', dependencies: 'ENG-11' }), // wedge on ENG-11
    story({ id: 'ENG-15', status: 'Planned', dependencies: 'ENG-12' }), // fine
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, [], []))
  assert.equal(kpis.decision.criticalDependencyCount, 2)
  const ids = kpis.decision.criticalDependencyPressure.map((p) => p.storyId)
  assert.deepEqual(ids.sort(), ['ENG-13', 'ENG-14'])
})

test('decision: recommendation ranks priority then age, excludes gates', () => {
  const stories = [
    story({ id: 'LOW-1', status: 'Ready', priority: 'Low' }),
    story({ id: 'CRIT-1', status: 'Ready', priority: 'Critical' }),
    story({ id: 'HIGH-1', status: 'Ready', priority: 'High' }),
  ]
  const items = [
    work({ id: 'w1', storyId: 'CRIT-1', state: 'Ready', queuedAt: '2026-08-22T04:00:00Z' }),
    work({ id: 'w2', storyId: 'HIGH-1', state: 'Ready', queuedAt: '2026-08-22T10:00:00Z' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  const rec = kpis.decision.recommended!
  assert.equal(rec.storyId, 'CRIT-1')
  assert.ok(rec.reasons.some((r) => r.includes('Critical')))
  assert.ok(rec.reasons.some((r) => r.includes('aged 8h')))
})

test('decision: age breaks priority ties (oldest eligible ready first)', () => {
  const stories = [
    story({ id: 'A-1', status: 'Ready', priority: 'High' }),
    story({ id: 'B-1', status: 'Ready', priority: 'High' }),
  ]
  const items = [
    work({ id: 'w1', storyId: 'A-1', state: 'Ready', queuedAt: '2026-08-22T10:00:00Z' }),
    work({ id: 'w2', storyId: 'B-1', state: 'Ready', queuedAt: '2026-08-22T02:00:00Z' }), // older
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.decision.recommended!.storyId, 'B-1')
})

test('decision: no recommendation when nothing is auto-dispatch eligible', () => {
  const stories = [story({ id: 'ENG-20', status: 'Ready' })]
  const items = [
    work({ id: 'w1', storyId: 'ENG-20', state: 'Ready', executionPolicy: 'Manual Only' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.decision.recommended, null)
  assert.equal(kpis.decision.autoDispatchEligibleCount, 0)
})

test('decision: recommendNextDispatch is stable and independent', () => {
  const stories = [story({ id: 'ENG-19', status: 'Ready', priority: 'High' })]
  const rec = recommendNextDispatch(
    {
      stories,
      workItems: [],
      pipeline: { readyWork: ['ENG-19'], gatedWork: [], blockedWork: [], nodes: [] },
      nowIso: NOW,
    },
  )
  assert.equal(rec?.storyId, 'ENG-19')
  assert.ok(rec!.reasons.some((r) => r.includes('High priority')))
})

// ---------------------------------------------------------------------------
// PIPPIN WATCH SOP — factory health
// ---------------------------------------------------------------------------

test('health: healthy with fresh heartbeat; isolated failure is NOT an emergency', () => {
  const stories = [
    story({ id: 'ENG-16', status: 'In Progress' }),
    story({ id: 'AUTH-05', status: 'Failed' }),
  ]
  const items = [
    work({ id: 'w1', storyId: 'ENG-16', state: 'Running', updatedAt: '2026-08-22T11:59:00Z' }), // fresh
    work({ id: 'w2', storyId: 'AUTH-05', state: 'Error', updatedAt: '2026-08-21T10:00:00Z' }), // historical residue
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.health.level, 'healthy')
  assert.match(kpis.health.summary, /Forge healthy; ENG-16 Running with fresh heartbeat/)
  assert.match(kpis.health.summary, /AUTH-05 is an isolated failed story; slot released, not blocking throughput/)
  assert.deepEqual(kpis.health.isolatedFailures, ['AUTH-05'])
})

test('health: stale active slot escalates to FACTORY UNHEALTHY', () => {
  const stories = [story({ id: 'ENG-16', status: 'In Progress' })]
  const items = [
    work({ id: 'w1', storyId: 'ENG-16', state: 'Running', updatedAt: '2026-08-22T09:00:00Z' }), // silent > 60m
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.health.level, 'escalate')
  assert.match(kpis.health.summary, /FACTORY UNHEALTHY/)
  assert.match(kpis.health.summary, /stale active slot/)
  assert.equal(kpis.health.activeHeartbeat?.stale, true)
})

test('health: scheduler wedge escalates when eligible work sits unclaimed on a free slot', () => {
  const stories = [story({ id: 'ENG-19', status: 'Ready' })]
  const items = [
    work({ id: 'w1', storyId: 'ENG-19', state: 'Ready', queuedAt: '2026-08-22T02:00:00Z' }), // 10h old, slot free
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.health.level, 'escalate')
  assert.match(kpis.health.summary, /scheduler wedge/)
  assert.equal(kpis.health.unclaimedEligible.length, 1)
})

test('health: fresh eligible ready work on a free slot is healthy, not a wedge', () => {
  const stories = [story({ id: 'ENG-19', status: 'Ready' })]
  const items = [
    work({ id: 'w1', storyId: 'ENG-19', state: 'Ready', queuedAt: '2026-08-22T11:55:00Z' }), // 5m old
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.health.level, 'healthy')
  assert.match(kpis.health.summary, /slot free with 1 dependency-ready command to claim/)
})

test('health: repeated failures on the same story trigger watch, not escalate', () => {
  const stories = [story({ id: 'ENG-08', status: 'Failed' })]
  const items = [
    work({ id: 'w1', storyId: 'ENG-08', state: 'Error', claimedBy: 'builder-flash' }),
    work({ id: 'w2', storyId: 'ENG-08', state: 'Error', claimedBy: 'builder-flash' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  assert.equal(kpis.health.level, 'watch')
  assert.match(kpis.health.summary, /repeated failure — ENG-08 failed 2 times/)
})

test('health: systemic failures (3+ distinct stories) trigger watch', () => {
  const runs = [
    run({ id: 'r1', storyId: 'ENG-01', resultStatus: 'Failed', startedAt: '2026-08-21T10:00:00Z' }),
    run({ id: 'r2', storyId: 'ENG-02', resultStatus: 'Failed', startedAt: '2026-08-21T11:00:00Z' }),
    run({ id: 'r3', storyId: 'ENG-03', resultStatus: 'Failed', startedAt: '2026-08-21T12:00:00Z' }),
  ]
  const kpis = buildFactoryKpis(kpiInput([], [], runs))
  assert.equal(kpis.health.level, 'watch')
  assert.match(kpis.health.summary, /3 distinct stories failed/)
})

test('health: paused command holds the slot — no false scheduler wedge', () => {
  const stories = [
    story({ id: 'ENG-16', status: 'In Progress' }),
    story({ id: 'ENG-19', status: 'Ready' }),
  ]
  const items = [
    work({ id: 'w1', storyId: 'ENG-16', state: 'Paused', updatedAt: '2026-08-22T11:00:00Z' }),
    work({ id: 'w2', storyId: 'ENG-19', state: 'Ready', queuedAt: '2026-08-22T02:00:00Z' }),
  ]
  const kpis = buildFactoryKpis(kpiInput(stories, items, []))
  // Paused holds the slot → replenishment is not free → no scheduler-wedge
  // escalation; the aged eligible work waits behind the pause.
  assert.equal(kpis.health.level, 'healthy')
  assert.equal(kpis.health.slotFree, false)
})

test('health: assessFactoryHealth is exported and deterministic', () => {
  const health = assessFactoryHealth({
    nowMs: Date.parse(NOW),
    nowIso: NOW,
    staleAfterMinutes: 60,
    schedulerWedgeMinutes: 60,
    workItems: [],
    pipeline: { readyWork: [], gatedWork: [], blockedWork: [], nodes: [] },
    eligibleStories: [],
    latestWork: new Map(),
    failedRuns: [],
  })
  assert.equal(health.level, 'healthy')
  assert.equal(health.summary, 'Forge healthy; slot free, no eligible ready work queued.')
})

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

test('priorityRankOf matches the ENG-16 priority ladder', () => {
  assert.equal(priorityRankOf('Critical'), 0)
  assert.equal(priorityRankOf('High'), 1)
  assert.equal(priorityRankOf('Medium'), 4)
  assert.equal(priorityRankOf('Later'), 6)
  assert.equal(priorityRankOf('Unknown-Priority'), 99)
})
