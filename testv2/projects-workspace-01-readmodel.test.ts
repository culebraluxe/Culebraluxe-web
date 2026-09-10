import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { Project } from '../services/project'
import type { WbsItem } from '../services/wbs'
import type { ActivityFeedEntry } from '../db/activity-feed'
import { mapRealProjectsToWorkspace } from '../ui/projects/service-projection'
import { toActivityFeedEntry } from '../db/activity-feed'

// PROJECTS-WORKSPACE-01 — the truthful workspace read model.
// Locks the frozen invariants: sort_order authority, completed-parent retention,
// stable-id secondary-view joins (never display-name equality), repository
// boundary normalization, and a discriminated load state.

function project(over: Partial<Project> = {}): Project {
  return {
    id: 'project-1',
    name: 'Listing Onboarding',
    owner: 'user-1',
    status: 'doing',
    description: '',
    areas: ['properties'],
    startsAt: null,
    endsAt: null,
    createdAt: '2026-09-09T00:00:00.000Z',
    updatedAt: '2026-09-09T00:00:00.000Z',
    projectType: null, playbookId: null, playbookVersion: null, personId: null, propertyId: null, contractId: null,
    ...over,
  }
}

function item(id: string, over: Partial<WbsItem> = {}): WbsItem {
  return {
    id,
    title: id,
    notes: '',
    category: 'management',
    status: 'open',
    projectId: 'project-1',
    parentId: null,
    dueAt: null,
    owner: null,
    order: null,
    entity: null,
    createdAt: '2026-09-09T00:00:00.000Z',
    updatedAt: '2026-09-09T00:00:00.000Z',
    ...over,
  }
}

function activity(over: Partial<ActivityFeedEntry> = {}): ActivityFeedEntry {
  return {
    id: 'a1',
    personId: null,
    dealId: null,
    channel: 'imessage',
    direction: 'inbound',
    occurredAt: '2026-01-01T00:00:00.000Z',
    occurredAtLabel: 'Jan 1',
    title: 'Hello',
    summary: null,
    personName: 'A',
    propertyName: null,
    dealPropertyName: null,
    ...over,
  }
}

function planFor(result: ReturnType<typeof mapRealProjectsToWorkspace>, id = 'project-1') {
  return result.poles.flatMap((pole) => pole.projects).find((candidate) => candidate.id === id)
}

test('sort_order is authoritative: ascending, nulls last, then dueAt, then id', () => {
  const data = mapRealProjectsToWorkspace([project()], [
    item('order-3', { order: 3 }),
    item('order-null-b', { order: null, dueAt: '2026-02-01T00:00:00.000Z' }),
    item('order-1', { order: 1 }),
    item('order-2', { order: 2 }),
    item('order-null-a', { order: null, dueAt: '2026-01-01T00:00:00.000Z' }),
  ])

  assert.deepEqual(
    planFor(data)?.workNodes.map((node) => node.id),
    ['order-1', 'order-2', 'order-3', 'order-null-a', 'order-null-b'],
  )
})

test('equal sort_order falls through to dueAt then id', () => {
  const data = mapRealProjectsToWorkspace([project()], [
    item('b', { order: 1, dueAt: '2026-01-02T00:00:00.000Z' }),
    item('a', { order: 1, dueAt: '2026-01-02T00:00:00.000Z' }),
    item('early', { order: 1, dueAt: '2026-01-01T00:00:00.000Z' }),
  ])

  assert.deepEqual(
    planFor(data)?.workNodes.map((node) => node.id),
    ['early', 'a', 'b'],
  )
})

test('completed and dismissed parents retain their open descendants', () => {
  const data = mapRealProjectsToWorkspace([project()], [
    item('done-parent', { status: 'done', order: 1 }),
    item('open-child', { parentId: 'done-parent', status: 'open', order: 1 }),
    item('dismissed-parent', { status: 'dismissed', order: 2 }),
    item('dismissed-child', { parentId: 'dismissed-parent', status: 'open', order: 1 }),
  ])

  const nodes = planFor(data)?.workNodes ?? []
  const done = nodes.find((node) => node.id === 'done-parent')
  const dismissed = nodes.find((node) => node.id === 'dismissed-parent')
  assert.equal(done?.children?.[0]?.id, 'open-child')
  assert.equal(dismissed?.children?.[0]?.id, 'dismissed-child')
  assert.equal(planFor(data)?.nextAction, 'open-child')
})

test('a project with no anchor yields empty panes and never matches globally', () => {
  const data = mapRealProjectsToWorkspace(
    [project()],
    [item('plain')],
    { 'property:property-9': 'Sunset Point' },
    [{ id: 'doc-1', title: 'Deed', state: 'ready', propertyId: 'property-9', createdAt: '2026-09-09T00:00:00.000Z' }],
    [activity({ id: 'a1', personId: 'person-9', propertyName: 'Sunset Point' })],
  )

  const plan = planFor(data)
  assert.deepEqual(plan?.documents, [])
  assert.deepEqual(plan?.activity, [])
  assert.equal(plan?.anchorSource, 'none')
  assert.deepEqual(plan?.provenance, { documents: 'unlinked', activity: 'unlinked', calendar: 'empty' })
})

test('activity joins by stable property id, never by display-name equality', () => {
  const data = mapRealProjectsToWorkspace(
    [project({ propertyId: 'property-1' })],
    [],
    { 'property:property-1': 'Sunset Point' },
    [],
    [
      activity({ id: 'by-id', propertyId: 'property-1', propertyName: 'A Different Name' }),
      activity({ id: 'name-only', propertyId: null, propertyName: 'Sunset Point' }),
    ],
  )

  const plan = planFor(data)
  assert.deepEqual(plan?.activity?.map((entry) => entry.id), ['by-id'])
})

test('documents join by stable property id and honour row-over-wbs precedence', () => {
  const data = mapRealProjectsToWorkspace(
    [project({ propertyId: 'property-row' })],
    [item('anchor', { entity: { type: 'property', id: 'property-wbs' } })],
    {},
    [
      { id: 'doc-row', title: 'Row', state: 'ready', propertyId: 'property-row', createdAt: '2026-09-09T00:00:00.000Z' },
      { id: 'doc-wbs', title: 'Wbs', state: 'ready', propertyId: 'property-wbs', createdAt: '2026-09-09T00:00:00.000Z' },
    ],
  )

  assert.deepEqual(planFor(data)?.documents?.map((document) => document.id), ['doc-row'])
  assert.equal(planFor(data)?.provenance?.documents, 'linked')
})

test('provenance reports empty when anchored but nothing matches', () => {
  const data = mapRealProjectsToWorkspace(
    [project({ personId: 'person-1', propertyId: 'property-1' })],
    [item('task', { dueAt: '2026-01-01T00:00:00.000Z' })],
  )

  const plan = planFor(data)
  assert.equal(plan?.provenance?.documents, 'empty')
  assert.equal(plan?.provenance?.activity, 'empty')
  assert.equal(plan?.provenance?.calendar, 'linked')
})

test('loadState is empty for zero projects and ready when projects exist', () => {
  const empty = mapRealProjectsToWorkspace([], [])
  const ready = mapRealProjectsToWorkspace([project()], [item('plain')])

  assert.equal(empty.loadState?.status, 'empty')
  assert.equal(ready.loadState?.status, 'ready')
})

test('work node inspector and actions come from canonical WBS facts, not invented labels', () => {
  const data = mapRealProjectsToWorkspace(
    [project({ propertyId: 'property-1' })],
    [item('contract-task', {
      category: 'contracts',
      notes: 'Waiting on countersignature.',
      owner: 'Lisa',
      entity: { type: 'property', id: 'property-1' },
    })],
    { 'property:property-1': 'Sunset Point' },
  )

  const node = planFor(data)?.workNodes[0]
  assert.equal(node?.type, 'contract')
  assert.deepEqual(node?.entity, { type: 'property', id: 'property-1' })
  assert.equal(node?.inspector?.summary, 'Waiting on countersignature.')
  assert.deepEqual(node?.inspector?.relatedItems, [
    { label: 'Sunset Point', caption: 'Property' },
    { label: 'Lisa', caption: 'Assignee' },
  ])
  assert.deepEqual(node?.actions, ['Open property'])
})

test('one selected project composes every pane from the page model', () => {
  const data = mapRealProjectsToWorkspace(
    [project({ personId: 'person-1', propertyId: 'property-1' })],
    [
      item('task-1', { order: 1, dueAt: '2026-02-01T00:00:00.000Z', notes: 'Do it', entity: { type: 'property', id: 'property-1' } }),
      item('task-2', { order: 2 }),
    ],
    { 'property:property-1': 'Sunset Point', 'person:person-1': 'Jessica Iverson' },
    [{ id: 'doc-1', title: 'Deed', state: 'ready', propertyId: 'property-1', createdAt: '2026-09-09T00:00:00.000Z' }],
    [activity({ id: 'a1', personId: 'person-1' })],
  )

  const plan = planFor(data)
  assert.ok(plan)
  assert.equal(plan.workNodes.length, 2)
  assert.equal(plan.calendarItems?.length, 1)
  assert.equal(plan.documents?.length, 1)
  assert.equal(plan.activity?.length, 1)
  assert.ok(plan.workNodes[0]?.inspector)
  assert.equal(plan.anchorSource, 'row')
})

test('repository boundary normalizes driver timestamps to ISO strings', () => {
  const entry = toActivityFeedEntry({
    id: 'a1',
    person_id: null,
    deal_id: null,
    channel: 'imessage',
    direction: null,
    occurred_at: new Date('2026-01-01T12:30:00.000Z'),
    occurred_at_label: 'Jan 1',
    title: 'Hello',
    summary: null,
    person_name: 'A',
    property_name: null,
    deal_property_name: null,
    property_id: 'property-1',
  })

  assert.equal(entry.occurredAt, '2026-01-01T12:30:00.000Z')
  assert.equal(entry.propertyId, 'property-1')
})
