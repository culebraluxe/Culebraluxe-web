import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  STORY_DOMAINS,
  TECH_SUBGROUPS,
  buildStoryBoardModel,
  statusBucket,
  storyDomainOf,
  storySubgroupOf,
  type StoryRecord,
} from '../../lib/storyboard-data'

// Pure-model tests for the five-domain rollup (NEXUS / MAIN / OPPS / SUPPORT /
// TECH). Completion math uses the stored completion (0..100), story status is
// categorical, execution state is the latest work-item/run, and the three axes
// never merge. rollup=false parents are counted but excluded from completion.

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
    // Complete forces 100; otherwise the caller provides an honest 0..100.
    completion:
      overrides.completion ?? (overrides.status === 'Complete' ? 100 : 0),
    rollup: overrides.rollup ?? true,
    plannedStartAt: overrides.plannedStartAt ?? null,
    actualStartAt: overrides.actualStartAt ?? null,
    completedAt: overrides.completedAt ?? null,
    createdAt: overrides.createdAt ?? '2026-08-21T00:00:00Z',
    updatedAt: overrides.updatedAt ?? '2026-08-21T00:00:00Z',
    execution: overrides.execution ?? null,
  }
}

// The canonical fixture mirrors the real PROD classification signals.
const FIXTURE = [
  // NEXUS
  story({ id: 'CRM-08', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'Complete' }),
  story({ id: 'INTAKE-01', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'In Progress', completion: 40 }),
  story({ id: 'DOC-01', workstream: 'TXN', operatingSurface: 'NEXUS', status: 'Planned', completion: 0 }),
  story({ id: 'CRM-15', workstream: 'TXN', operatingSurface: 'NEXUS', status: 'Partial', completion: 50 }),
  story({ id: 'PORTAL-03', workstream: 'PORTAL', operatingSurface: 'NEXUS', status: 'Ready', completion: 20 }),
  // MAIN
  story({ id: 'PX-21', workstream: 'PUBLIC', operatingSurface: 'NEXUS', status: 'Complete' }),
  story({ id: 'POLISH-02', workstream: 'PUBLIC', operatingSurface: 'NEXUS', status: 'Hold', completion: 30 }),
  story({ id: 'PLAT-01', workstream: 'CONTENT', operatingSurface: 'OPS', status: 'Complete' }),
  // OPPS
  story({ id: 'OPS-02', workstream: 'ADMIN', operatingSurface: 'OPS', status: 'Complete' }),
  story({ id: 'CRM-17', workstream: 'CRM', operatingSurface: 'OPS', status: 'Complete' }),
  // SUPPORT
  story({ id: 'AUTH-04', workstream: 'AUTH', operatingSurface: 'SUPPORT', status: 'Complete' }),
  story({ id: 'OPS-09', workstream: 'HARDEN', operatingSurface: 'SUPPORT', status: 'Ready', completion: 10 }),
  // TECH
  story({ id: 'ENG-21', workstream: 'HARDEN', operatingSurface: 'TECH', status: 'Ready', completion: 5 }),
  story({ id: 'ARCH-HANDOFF', workstream: 'ARCH', operatingSurface: 'TECH', status: 'Complete' }),
  story({ id: 'CRM-14B', workstream: 'TXN', operatingSurface: 'TECH', status: 'Complete' }),
]

test('domain classifier maps the five domains deterministically', () => {
  assert.equal(storyDomainOf(FIXTURE[0]), 'NEXUS')
  assert.equal(storyDomainOf(FIXTURE[5]), 'MAIN')
  assert.equal(storyDomainOf(FIXTURE[8]), 'OPPS')
  assert.equal(storyDomainOf(FIXTURE[10]), 'SUPPORT')
  assert.equal(storyDomainOf(FIXTURE[12]), 'TECH')
  // AUTH prefix overrides a TECH surface (SUPPORT owns auth/security).
  const authTouchedTech = story({ id: 'AUTH-01', workstream: 'AUTH', operatingSurface: 'TECH', status: 'Complete' })
  assert.equal(storyDomainOf(authTouchedTech), 'SUPPORT')
  // No operating surface and no MAIN/AUTH signal -> UNCLASSIFIED, never guessed.
  assert.equal(storyDomainOf(story({ id: 'ZZ-9', operatingSurface: null, status: 'Planned' })), 'UNCLASSIFIED')
})

test('subgroup classifier assigns representative subgroups', () => {
  assert.equal(storySubgroupOf(FIXTURE[0]), 'CRM / Intake')
  assert.equal(storySubgroupOf(FIXTURE[1]), 'CRM / Intake')
  assert.equal(storySubgroupOf(FIXTURE[2]), 'Forms / DOC')
  assert.equal(storySubgroupOf(FIXTURE[3]), 'Deals / TXN')
  assert.equal(storySubgroupOf(FIXTURE[4]), 'Portal / Relationship')
  assert.equal(storySubgroupOf(FIXTURE[5]), 'PX / Public')
  assert.equal(storySubgroupOf(FIXTURE[6]), 'Public Site / Polish')
  assert.equal(storySubgroupOf(FIXTURE[7]), 'Property / Platform Data')
  assert.equal(storySubgroupOf(FIXTURE[8]), 'Admin / Process')
  assert.equal(storySubgroupOf(FIXTURE[9]), 'CRM / Intake')
  assert.equal(storySubgroupOf(FIXTURE[10]), 'AUTH / Security')
  assert.equal(storySubgroupOf(FIXTURE[11]), 'Support / Ops')
  assert.equal(storySubgroupOf(FIXTURE[12]), 'FRAMEWORKS')
  assert.equal(storySubgroupOf(FIXTURE[13]), 'ARCH')
  assert.equal(storySubgroupOf(FIXTURE[14]), 'WORKFLOW ENGINE')
  // Representative TECH taxonomy covers every canonical subgroup.
  assert.equal(storySubgroupOf(story({ id: 'MQ-01', workstream: 'HARDEN', operatingSurface: 'TECH', status: 'Planned' })), 'MQ MINI')
  assert.equal(storySubgroupOf(story({ id: 'ALERT-01', workstream: 'HARDEN', operatingSurface: 'TECH', status: 'Planned' })), 'ALERTS')
  assert.equal(storySubgroupOf(story({ id: 'FORGE-01', workstream: 'HARDEN', operatingSurface: 'TECH', status: 'Planned' })), 'FRAMEWORKS')
})

test('domain totals reconcile: five domains sum to total story count', () => {
  const model = buildStoryBoardModel(FIXTURE)
  assert.equal(model.totalStories, 15)
  const sum = model.domains.reduce((total, d) => total + d.storyCount, 0)
  assert.equal(sum, model.totalStories)
  assert.equal(model.unclassifiedCount, 0)
  const counts = Object.fromEntries(model.domains.map((d) => [d.domain, d.storyCount]))
  assert.deepEqual(counts, { NEXUS: 5, MAIN: 3, OPPS: 2, SUPPORT: 2, TECH: 3 })
})

test('domain completion is the AVG of stored completion over CURRENT-SCOPE rollup stories', () => {
  const model = buildStoryBoardModel(FIXTURE)
  const nexus = model.domains.find((d) => d.domain === 'NEXUS')!
  // DOC-01 is Planned (Backlog) — excluded from current-scope completion.
  assert.equal(nexus.completionPercent, 52.5) // (100 + 40 + 50 + 20) / 4
  const tech = model.domains.find((d) => d.domain === 'TECH')!
  assert.equal(tech.completionPercent, 68.3) // (5 + 100 + 100) / 3
  // Status is categorical - a status change alone never alters completion.
  const asBlocked = buildStoryBoardModel(
    FIXTURE.map((s) => (s.id === 'CRM-15' ? { ...s, status: 'Blocked' } : s)),
  )
  assert.equal(
    asBlocked.domains.find((d) => d.domain === 'NEXUS')!.completionPercent,
    52.5,
  )
})



test('rollup=false parents are counted but carry no completion weight', () => {
  const model = buildStoryBoardModel([
    story({ id: 'CRM-16', workstream: 'TXN', operatingSurface: 'NEXUS', status: 'Complete', rollup: false }),
    story({ id: 'CRM-19', workstream: 'TXN', operatingSurface: 'NEXUS', status: 'Partial', completion: 100 }),
    story({ id: 'CRM-20', workstream: 'TXN', operatingSurface: 'NEXUS', status: 'Planned', completion: 0 }),
  ])
  const nexus = model.domains.find((d) => d.domain === 'NEXUS')!
  assert.equal(nexus.storyCount, 3)
  assert.equal(nexus.completeCount, 1)
  // CRM-16 (rollup=false) and CRM-20 (Planned/Backlog) are excluded from
  // completion; only CRM-19 (Partial, current scope) counts: (100) / 1.
  assert.equal(nexus.completionPercent, 100)
})

test('net-net is the simple mean of the five domain completion percents', () => {
  const model = buildStoryBoardModel(FIXTURE)
  const percents = model.domains.map((d) => d.completionPercent)
  const expected =
    Math.round((percents.reduce((sum, p) => sum + p, 0) / percents.length) * 10) / 10
  assert.equal(model.netNet, expected)
  assert.equal(model.netNet, 70.5)
})

test('current-scope completion excludes Backlog (Planned) stories from both terms', () => {
  const model = buildStoryBoardModel([
    story({ id: 'B-1', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'Complete' }),
    story({ id: 'B-2', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'Planned', completion: 0 }),
  ])
  const nexus = model.domains.find((d) => d.domain === 'NEXUS')!
  assert.equal(nexus.storyCount, 2) // the Planned story still appears in the panel
  assert.equal(nexus.completionPercent, 100) // only the current-scope story counts
})

test('current-scope completion excludes Next Version (Deferred) stories from both terms', () => {
  const model = buildStoryBoardModel([
    story({ id: 'N-1', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'Complete' }),
    story({ id: 'N-2', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'Deferred', completion: 0 }),
  ])
  const nexus = model.domains.find((d) => d.domain === 'NEXUS')!
  assert.equal(nexus.storyCount, 2)
  assert.equal(nexus.completionPercent, 100)
})

test('Open stories reduce completion; Backlog/Next-Version do not', () => {
  const base = [
    story({ id: 'O-1', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'Complete' }),
    story({ id: 'O-2', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'In Progress', completion: 40 }),
  ]
  const openOnly = buildStoryBoardModel(base)
  assert.equal(openOnly.domains.find((d) => d.domain === 'NEXUS')!.completionPercent, 70) // (100+40)/2

  // Adding Planned + Deferred (both 0) must NOT lower current completion.
  const withBacklog = buildStoryBoardModel([
    ...base,
    story({ id: 'O-3', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'Planned', completion: 0 }),
    story({ id: 'O-4', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'Deferred', completion: 0 }),
  ])
  assert.equal(withBacklog.domains.find((d) => d.domain === 'NEXUS')!.completionPercent, 70)

  // A Partial OPEN story still counts (reduces the average) and is not excluded.
  const withPartial = buildStoryBoardModel([
    ...base,
    story({ id: 'O-5', workstream: 'CRM', operatingSurface: 'NEXUS', status: 'Partial', completion: 20 }),
  ])
  assert.equal(withPartial.domains.find((d) => d.domain === 'NEXUS')!.completionPercent, 53.3) // (100+40+20)/3
})

test('execution counts come from the latest work item, distinct from status', () => {
  const running = story({
    id: 'ENG-22', workstream: 'HARDEN', operatingSurface: 'TECH', status: 'Ready', completion: 5,
    execution: { workItemState: 'Running', latestRunResult: null, latestRunAt: '2026-08-21T10:00:00Z' },
  })
  const errored = story({
    id: 'ENG-23', workstream: 'HARDEN', operatingSurface: 'TECH', status: 'Partial', completion: 30,
    execution: { workItemState: 'Error', latestRunResult: 'Failed', latestRunAt: '2026-08-21T09:00:00Z' },
  })
  const done = story({
    id: 'ENG-24', workstream: 'HARDEN', operatingSurface: 'TECH', status: 'Complete', completion: 100,
    execution: { workItemState: 'Done', latestRunResult: 'Complete', latestRunAt: '2026-08-20T10:00:00Z' },
  })
  const model = buildStoryBoardModel([running, errored, done])
  assert.equal(model.totalRunning, 1)
  assert.equal(model.totalError, 1)
  assert.equal(model.totalComplete, 1)
  // Story status 'Ready' and execution state 'Running' stay distinct.
  const tech = model.domains.find((d) => d.domain === 'TECH')!
  assert.equal(tech.readyStoryCount, 1)
  assert.equal(tech.runningCount, 1)
  assert.equal(tech.errorCount, 1)
})

test('TECH always exposes the canonical five subgroups (empty ones included)', () => {
  const model = buildStoryBoardModel(FIXTURE)
  const tech = model.domains.find((d) => d.domain === 'TECH')!
  assert.deepEqual(
    tech.subgroups.map((s) => s.subgroup),
    [...TECH_SUBGROUPS],
  )
  const mq = tech.subgroups.find((s) => s.subgroup === 'MQ MINI')!
  assert.equal(mq.storyCount, 0)
  const arch = tech.subgroups.find((s) => s.subgroup === 'ARCH')!
  assert.equal(arch.storyCount, 1)
})

test('statusBucket categorizes the nine controlled statuses', () => {
  assert.equal(statusBucket('Complete'), 'complete')
  assert.equal(statusBucket('In Progress'), 'partial')
  assert.equal(statusBucket('Partial'), 'partial')
  assert.equal(statusBucket('Planned'), 'open')
  assert.equal(statusBucket('Ready'), 'open')
  assert.equal(statusBucket('Deferred'), 'open')
  assert.equal(statusBucket('Hold'), 'open')
  assert.equal(statusBucket('Failed'), 'open')
  assert.equal(statusBucket('Blocked'), 'blocked')
})

test('changing completion changes domain completion and net-net', () => {
  const before = buildStoryBoardModel([story({ id: 'Z-1', operatingSurface: 'NEXUS', status: 'Partial', completion: 20 })])
  const after = buildStoryBoardModel([story({ id: 'Z-1', operatingSurface: 'NEXUS', status: 'Partial', completion: 80 })])
  assert.equal(before.domains[0].completionPercent, 20)
  assert.equal(after.domains[0].completionPercent, 80)
  assert.notEqual(after.netNet, before.netNet)
})

test('the five canonical domains are the only top-level rollup parents', () => {
  assert.deepEqual([...STORY_DOMAINS], ['NEXUS', 'MAIN', 'OPPS', 'SUPPORT', 'TECH'])
})
