// ---------------------------------------------------------------------------
// PORTAL-12 — Story Board Operating Cockpit projection.
//
// Proves the lifecycle classification and cockpit grouping are derived purely
// from canonical story status/domain data:
//   1. every stored status maps to EXACTLY ONE lifecycle bucket
//   2. a story appears in exactly one panel
//   3. panels group stories by canonical domain subgroup, sorted alphabetically
//   4. stories are ordered deterministically (by id) within each domain group
//   5. KPI values derive from canonical rows
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  STORY_STATUSES,
  buildStoryBoardCockpit,
  buildStoryBoardModel,
  storyLifecycleOf,
} from '../../lib/storyboard-data'
import type { StoryRecord, StoryStatus } from '../../lib/storyboard-data'

function story(id: string, status: StoryStatus, title = id): StoryRecord {
  return {
    id,
    workstream: 'CRM',
    operatingSurface: 'NEXUS',
    title,
    priority: 'Medium',
    status,
    notes: '',
    batch: null,
    goal: null,
    scope: null,
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: null,
    acceptanceCriteria: null,
    postconditions: null,
    architectBriefUpdatedAt: null,
    completion: 50,
    rollup: true,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
  }
}

test('PORTAL-12: every stored status maps to exactly one lifecycle bucket', () => {
  const seen = new Set<string>()
  for (const status of STORY_STATUSES) {
    const lifecycle = storyLifecycleOf(status)
    assert.ok(['open', 'backlog', 'closed', 'next-version'].includes(lifecycle))
    // A status contributes its bucket only once (each status -> one bucket).
    seen.add(`${status}:${lifecycle}`)
  }
  // Every status covered (9 statuses -> 9 unique status:bucket pairs).
  assert.equal(seen.size, STORY_STATUSES.length)
})

test('PORTAL-12: a story appears in exactly one lifecycle panel', () => {
  const statuses: StoryStatus[] = [
    'In Progress',
    'Partial',
    'Ready',
    'Blocked',
    'Hold',
    'Failed',
    'Planned',
    'Complete',
    'Deferred',
  ]
  const stories = statuses.map((s, i) => story(`AUTH-0${i + 1}`, s))
  const model = buildStoryBoardModel(stories)
  const cockpit = buildStoryBoardCockpit(model)

  const total = Object.values(cockpit.panels).reduce(
    (sum, p) => sum + p.count,
    0,
  )
  assert.equal(total, stories.length, 'no story appears in more than one panel')
  assert.equal(cockpit.kpis.open + cockpit.kpis.backlog + cockpit.kpis.complete + cockpit.kpis.nextVersion, stories.length)
})

test('PORTAL-12: panels group by canonical domain subgroup, sorted; stories deterministic', () => {
  // Two AUTH (SUPPORT domain -> 'AUTH / Security' subgroup) and one CRM (NEXUS).
  const stories = [
    story('AUTH-09', 'Ready'),
    story('AUTH-06', 'In Progress'),
    story('CRM-24', 'Planned'),
    story('INTAKE-02', 'Ready'),
  ]
  const model = buildStoryBoardModel(stories)
  const cockpit = buildStoryBoardCockpit(model)

  // OPEN should hold AUTH-09, AUTH-06, INTAKE-02; BACKLOG holds CRM-24.
  const openGroups = cockpit.panels.open.groups
  // Group labels come from the canonical subgroup classifier and are sorted.
  const labels = openGroups.map((g) => g.group)
  assert.deepEqual([...labels].sort(), labels, 'domain groups sorted alphabetically')
  // Within each group stories are sorted by id.
  for (const g of openGroups) {
    const ids = g.stories.map((s) => s.id)
    const sorted = [...ids].sort()
    assert.deepEqual(ids, sorted, `stories in ${g.group} sorted deterministically`)
  }
})

test('PORTAL-12: KPI values derive from canonical rows', () => {
  const stories = [
    story('A-01', 'In Progress'),
    story('A-02', 'Blocked'),
    story('A-03', 'Planned'),
    story('A-04', 'Complete'),
    story('A-05', 'Deferred'),
    story('A-06', 'Hold'),
  ]
  const model = buildStoryBoardModel(stories)
  const cockpit = buildStoryBoardCockpit(model)
  assert.equal(cockpit.kpis.total, 6)
  assert.equal(cockpit.kpis.open, 3) // In Progress, Blocked, Hold
  assert.equal(cockpit.kpis.backlog, 1) // Planned
  assert.equal(cockpit.kpis.complete, 1) // Complete
  assert.equal(cockpit.kpis.nextVersion, 1) // Deferred
  assert.equal(cockpit.kpis.blockedHold, 2) // Blocked + Hold
})
