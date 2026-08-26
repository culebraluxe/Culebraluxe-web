// ---------------------------------------------------------------------------
// PORTAL-13 — TECH Engineering Cockpit: active queue, run history, copy packets.
//
// Proves the cockpit's data semantics without a live browser:
//   A  active-work flag never changes story status
//   B  active queue contains only explicitly selected stories
//   C  active queue ordering is deterministic
//   D  selected parent detail uses canonical storyboard_story fields
//   E  run grid is scoped to the selected story only
//   F  run snapshot stays distinct from current parent data
//   G  one story can carry multiple run rows
//   H  copy-story packet includes architecture fields
//   I  copy-run packet includes the frozen snapshot fields
//   J  lifecycle bucket classification is unchanged
//   K  cockpit KPI counts remain canonical
//   L  TECH route remains tech.access protected
// ---------------------------------------------------------------------------

import { test, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import { setDatabaseTestExecutor } from '../../db/client'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import { setActiveWork, listStoryRuns } from '../../db/storyboard'
import {
  buildActiveQueue,
  buildStoryBoardCockpit,
  buildStoryBoardModel,
  formatRunPacket,
  formatStoryPacket,
  storyLifecycleOf,
  type StoryRecord,
  type StoryStatus,
} from '../../lib/storyboard-data'
import { resolvePortalAccess } from '../../lib/auth/require-portal-access'
import type { SessionAdapter } from '../../lib/auth/session-adapter'

afterEach(() => setDatabaseTestExecutor(null))

function story(
  id: string,
  status: StoryStatus,
  isActiveWork = false,
  order: number | null = null,
): StoryRecord {
  return {
    id,
    workstream: 'AUTH',
    operatingSurface: 'SUPPORT',
    title: `Story ${id}`,
    priority: 'High',
    status,
    notes: 'note',
    batch: null,
    goal: 'Goal for ' + id,
    scope: 'Scope',
    dependencies: null,
    preconditions: 'Pre',
    architectBrief: 'Arch brief for ' + id,
    contextRefs: 'refs',
    acceptanceCriteria: 'AC for ' + id,
    postconditions: 'Post',
    architectBriefUpdatedAt: null,
    completion: 50,
    rollup: true,
    isActiveWork,
    activeWorkOrder: order,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
  }
}

/** Capturing executor that returns empty rows. */
function captureExecutor(captured: string[]): QueryExecutor {
  return async (strings) => {
    captured.push(strings.join('?'))
    return []
  }
}

function runRow(id: string, storyId: string, startedAt: string): QueryRow {
  return {
    id,
    story_id: storyId,
    started_at: startedAt,
    ended_at: null,
    result_status: 'Complete',
    run_type: 'IMPLEMENTATION',
    agent_runtime: 'Cline',
    completion: 100,
    notes: 'run note',
    commit_hash: 'abc123',
    tests_summary: '12 passed',
    execution_environment: 'DEV',
    goal_snapshot: 'frozen goal',
    preconditions_snapshot: 'frozen pre',
    architect_brief_snapshot: 'frozen arch',
    context_refs_snapshot: 'frozen refs',
    acceptance_criteria_snapshot: 'frozen ac',
    postconditions_snapshot: 'frozen post',
    created_at: startedAt,
    updated_at: startedAt,
  }
}

test('PORTAL-13 A: active-work flag never changes story status', async () => {
  const captured: string[] = []
  await setActiveWork('AUTH-09', true, captureExecutor(captured))
  await setActiveWork('AUTH-09', false, captureExecutor(captured))
  for (const sql of captured) {
    assert.ok(/is_active_work/.test(sql), 'update touches is_active_work')
    assert.ok(/active_work_order/.test(sql), 'update touches active_work_order')
    assert.ok(!/set\s+status\s*=/.test(sql), 'update must NOT set status')
  }
})

test('PORTAL-13 B: active queue contains only explicitly selected stories', () => {
  const stories = [
    story('A-1', 'In Progress', true, 1),
    story('A-2', 'Planned', true, 2),
    story('A-3', 'Planned', false),
    story('A-4', 'Complete', false),
  ]
  const queue = buildActiveQueue(stories)
  assert.deepEqual(queue.map((s) => s.id), ['A-1', 'A-2'])
})

test('PORTAL-13 C: active queue ordering is deterministic', () => {
  const stories = [
    story('C-3', 'In Progress', true, 3),
    story('C-1', 'In Progress', true, 1),
    story('C-2', 'In Progress', true, null),
    story('C-4', 'In Progress', true, 2),
  ]
  const queue = buildActiveQueue(stories)
  assert.deepEqual(queue.map((s) => s.id), ['C-1', 'C-4', 'C-3', 'C-2'])
})

test('PORTAL-13 D: selected parent detail uses canonical storyboard_story fields', () => {
  const s = story('AUTH-09', 'In Progress')
  const packet = formatStoryPacket(s)
  assert.ok(packet.includes('## Architecture Brief'))
  assert.ok(packet.includes('Arch brief for AUTH-09'))
  assert.ok(packet.includes('## Goal'))
  assert.ok(packet.includes('Goal for AUTH-09'))
  assert.ok(packet.includes('## Acceptance Criteria'))
  assert.ok(packet.includes('AC for AUTH-09'))
})

test('PORTAL-13 E: run grid is scoped to the selected story only', async () => {
  const captured: string[] = []
  await listStoryRuns('AUTH-09', captureExecutor(captured))
  const last = captured[captured.length - 1]
  assert.ok(/where\s+story_id\s*=/.test(last), 'run query scoped by story_id')
  assert.ok(!/where\s+story_id\s*=\s*.*OR/i.test(last), 'single story scope')
})

test('PORTAL-13 G: one story can carry multiple run rows', async () => {
  const rows = [
    runRow('run-1', 'AUTH-09', '2026-01-01T00:00:00Z'),
    runRow('run-2', 'AUTH-09', '2026-01-02T00:00:00Z'),
    runRow('run-3', 'AUTH-09', '2026-01-03T00:00:00Z'),
  ]
  setDatabaseTestExecutor((async () => rows) as unknown as QueryExecutor)
  const runs = await listStoryRuns('AUTH-09')
  assert.equal(runs.length, 3)
  assert.ok(runs.every((r) => r.storyId === 'AUTH-09'))
})

test('PORTAL-13 F: run snapshot stays distinct from current parent data', () => {
  const s = story('AUTH-09', 'In Progress')
  const run = {
    id: 'run-1',
    startedAt: '2026-01-01T00:00:00Z',
    endedAt: null,
    resultStatus: 'Complete',
    completion: 100,
    notes: 'run note',
    commitHash: 'abc123',
    testsSummary: '12 passed',
    executionEnvironment: 'DEV',
    runType: 'IMPLEMENTATION',
    agentRuntime: 'Cline',
    goalSnapshot: 'FROZEN-goal',
    architectBriefSnapshot: 'FROZEN-arch',
    preconditionsSnapshot: 'FROZEN-pre',
    acceptanceCriteriaSnapshot: 'FROZEN-ac',
    postconditionsSnapshot: 'FROZEN-post',
    contextRefsSnapshot: 'FROZEN-refs',
  }
  const storyPacket = formatStoryPacket(s)
  const runPacket = formatRunPacket(s.id, run)
  // Current story uses plain sections; the run packet uses frozen snapshots.
  assert.ok(runPacket.includes('Frozen Architecture Brief'))
  assert.ok(runPacket.includes('FROZEN-arch'))
  assert.ok(storyPacket.includes('## Architecture Brief'))
  assert.ok(!storyPacket.includes('Frozen Architecture Brief'))
})

test('PORTAL-13 H: copy-story packet includes architecture fields', () => {
  const s = story('AUTH-09', 'In Progress')
  const packet = formatStoryPacket(s)
  for (const label of [
    'Goal',
    'Architecture Brief',
    'Scope',
    'Preconditions',
    'Acceptance Criteria',
    'Postconditions',
    'Context / References',
    'Notes',
  ]) {
    assert.ok(packet.includes(`## ${label}`), `packet includes ${label}`)
  }
})

test('PORTAL-13 I: copy-run packet includes frozen snapshot fields', () => {
  const run = {
    id: 'run-1',
    startedAt: '2026-01-01T00:00:00Z',
    endedAt: null,
    resultStatus: 'Complete',
    completion: 100,
    notes: 'run note',
    commitHash: 'abc123',
    testsSummary: '12 passed',
    executionEnvironment: 'DEV',
    runType: 'IMPLEMENTATION',
    agentRuntime: 'Cline',
    goalSnapshot: 'g',
    architectBriefSnapshot: 'a',
    preconditionsSnapshot: 'p',
    acceptanceCriteriaSnapshot: 'ac',
    postconditionsSnapshot: 'po',
    contextRefsSnapshot: 'r',
  }
  const packet = formatRunPacket('AUTH-09', run)
  for (const label of [
    'Frozen Goal',
    'Frozen Architecture Brief',
    'Frozen Preconditions',
    'Frozen Acceptance Criteria',
    'Frozen Postconditions',
    'Frozen Context / References',
  ]) {
    assert.ok(packet.includes(label), `run packet includes ${label}`)
  }
  assert.ok(packet.includes('Pass / type: IMPLEMENTATION'))
  assert.ok(packet.includes('Agent / runtime: Cline'))
})

test('PORTAL-13 J: lifecycle bucket classification is unchanged', () => {
  assert.equal(storyLifecycleOf('In Progress'), 'open')
  assert.equal(storyLifecycleOf('Planned'), 'backlog')
  assert.equal(storyLifecycleOf('Complete'), 'closed')
  assert.equal(storyLifecycleOf('Deferred'), 'next-version')
  assert.equal(storyLifecycleOf('Blocked'), 'open')
  assert.equal(storyLifecycleOf('Hold'), 'open')
})

test('PORTAL-13 K: cockpit KPI counts remain canonical', () => {
  const stories = [
    story('A-1', 'In Progress'),
    story('A-2', 'Planned'),
    story('A-3', 'Complete'),
    story('A-4', 'Deferred'),
  ]
  const model = buildStoryBoardModel(stories)
  const cockpit = buildStoryBoardCockpit(model)
  assert.equal(cockpit.kpis.total, 4)
  assert.equal(cockpit.kpis.open, 1)
  assert.equal(cockpit.kpis.backlog, 1)
  assert.equal(cockpit.kpis.complete, 1)
  assert.equal(cockpit.kpis.nextVersion, 1)
})

test('PORTAL-13 L: TECH route remains tech.access protected', async () => {
  const src = await readFile(
    new URL('../../app/portal/tech/page.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(
    src.includes('resolvePortalAccess') && src.includes('tech.access'),
    'tech page enforces tech.access server-side',
  )
  assert.ok(src.includes('redirect') && src.includes('redirectTo'), 'denial redirects')
})
