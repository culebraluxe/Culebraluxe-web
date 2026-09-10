import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { Project } from '../services/project'
import type { WbsItem } from '../services/wbs'
import { mapRealProjectsToWorkspace } from '../ui/projects/service-projection'

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

test('Projects projection uses complete WBS truth for progress, status, and sibling order', () => {
  const data = mapRealProjectsToWorkspace(
    [project()],
    [
      item('open-second', { title: 'Second', order: 2 }),
      item('done-first', { title: 'First', order: 1, status: 'done' }),
      item('dismissed-third', { title: 'Removed', order: 3, status: 'dismissed' }),
    ],
  )

  const plan = data.poles[0]?.projects[0]
  assert.ok(plan)
  assert.equal(plan.progress, 50, 'dismissed work is not complete and is excluded from the active denominator')
  assert.equal(plan.status, 'active')
  assert.equal(plan.phaseLabel, 'In progress')
  assert.deepEqual(plan.workNodes.map((node) => node.title), ['First', 'Second', 'Removed'])
  assert.deepEqual(plan.workNodes.map((node) => node.status), ['complete', 'not-started', 'dismissed'])
})

test('Projects projection preserves an open child beneath a completed parent', () => {
  const data = mapRealProjectsToWorkspace(
    [project({ status: 'open' })],
    [
      item('parent', { title: 'Listing Agreement', status: 'done', order: 1 }),
      item('child', { title: 'Seller Signature', parentId: 'parent', status: 'open', order: 1 }),
    ],
  )

  const plan = data.poles[0]?.projects[0]
  assert.ok(plan)
  assert.equal(plan.status, 'planning')
  assert.equal(plan.phaseLabel, 'Open')
  assert.equal(plan.workNodes[0]?.status, 'complete')
  assert.equal(plan.workNodes[0]?.children?.[0]?.title, 'Seller Signature')
  assert.equal(plan.workNodes[0]?.children?.[0]?.status, 'not-started')
})

test('Projects projection does not hardcode completed or archived Projects as active', () => {
  const complete = mapRealProjectsToWorkspace([project({ status: 'done' })], [item('done', { status: 'done' })])
  const archived = mapRealProjectsToWorkspace([project({ status: 'archived' })], [item('open')])

  assert.equal(complete.poles[0]?.projects[0]?.status, 'complete')
  assert.equal(complete.poles[0]?.projects[0]?.phaseLabel, 'Complete')
  assert.equal(archived.poles[0]?.projects[0]?.status, 'archived')
  assert.equal(archived.poles[0]?.projects[0]?.phaseLabel, 'Archived')
})

test('Projects projection exposes persisted playbook identity, context, and next action', () => {
  const data = mapRealProjectsToWorkspace(
    [project({ projectType: 'listing', playbookId: 'listing-onboarding', playbookVersion: 1, personId: 'person-test', propertyId: 'property-test' })],
    [item('done', { status: 'done', order: 1 }), item('sign', { title: 'Seller Signature', order: 2 })],
    { 'person:person-test': 'Synthetic Seller', 'property:property-test': 'Synthetic Property' },
  )
  const plan = data.poles[0]?.projects[0]
  assert.ok(plan)
  assert.equal(plan.kind, 'LISTING')
  assert.equal(plan.playbookId, 'listing-onboarding')
  assert.deepEqual(plan.contextLabels, ['Synthetic Seller', 'Synthetic Property'])
  assert.equal(plan.nextAction, 'Seller Signature')
})

test('Projects projection scopes activity to the project person or resolved property', () => {
  const projectFixture = project({ personId: 'person-1', propertyId: 'property-1' })
  const result = mapRealProjectsToWorkspace([projectFixture], [], { 'property:property-1': 'Sunset Point' }, [], [
    { id: 'a1', personId: 'person-1', dealId: null, channel: 'imessage', direction: 'inbound', occurredAt: '2026-01-01', occurredAtLabel: 'Jan 1', title: 'Hello', summary: null, personName: 'A', propertyName: null, dealPropertyName: null },
    { id: 'a2', personId: 'other', dealId: null, channel: 'email', direction: 'outbound', occurredAt: '2026-01-02', occurredAtLabel: 'Jan 2', title: 'Other', summary: null, personName: 'B', propertyName: null, dealPropertyName: null },
  ])
  const plan = result.poles.flatMap((pole) => pole.projects).find((candidate) => candidate.id === projectFixture.id)
  assert.deepEqual(plan?.activity?.map((entry) => entry.id), ['a1'])
})

function planFor(result: ReturnType<typeof mapRealProjectsToWorkspace>, id = 'project-1') {
  return result.poles.flatMap((pole) => pole.projects).find((candidate) => candidate.id === id)
}

test('Projects projection attaches WBS property-anchored documents when the row has no propertyId', () => {
  const data = mapRealProjectsToWorkspace(
    [project()],
    [item('anchor', { entity: { type: 'property', id: 'property-wbs' } })],
    {},
    [{ id: 'doc-1', title: 'Deed', state: 'ready', propertyId: 'property-wbs', createdAt: '2026-09-09T00:00:00.000Z' }],
  )

  const plan = planFor(data)
  assert.ok(plan)
  assert.deepEqual(plan.documents?.map((document) => document.id), ['doc-1'])
})

test('Projects projection matches WBS person-anchored activity when the row has no personId', () => {
  const data = mapRealProjectsToWorkspace(
    [project()],
    [item('anchor', { entity: { type: 'person', id: 'person-wbs' } })],
    {},
    [],
    [
      { id: 'a1', personId: 'person-wbs', dealId: null, channel: 'imessage', direction: 'inbound', occurredAt: '2026-01-01', occurredAtLabel: 'Jan 1', title: 'Hello', summary: null, personName: 'A', propertyName: null, dealPropertyName: null },
      { id: 'a2', personId: 'other', dealId: null, channel: 'email', direction: 'outbound', occurredAt: '2026-01-02', occurredAtLabel: 'Jan 2', title: 'Other', summary: null, personName: 'B', propertyName: null, dealPropertyName: null },
    ],
  )

  const plan = planFor(data)
  assert.ok(plan)
  assert.deepEqual(plan.activity?.map((entry) => entry.id), ['a1'])
})

test('Projects projection projects empty documents and activity when neither row nor WBS anchors exist', () => {
  const data = mapRealProjectsToWorkspace(
    [project()],
    [item('plain')],
    { 'property:property-9': 'Sunset Point' },
    [{ id: 'doc-1', title: 'Deed', state: 'ready', propertyId: 'property-9', createdAt: '2026-09-09T00:00:00.000Z' }],
    [
      { id: 'a1', personId: 'person-9', dealId: null, channel: 'imessage', direction: 'inbound', occurredAt: '2026-01-01', occurredAtLabel: 'Jan 1', title: 'Hello', summary: null, personName: 'A', propertyName: 'Sunset Point', dealPropertyName: null },
    ],
  )

  const plan = planFor(data)
  assert.ok(plan)
  assert.deepEqual(plan.documents, [])
  assert.deepEqual(plan.activity, [])
})

test('Projects projection row anchors win over WBS anchors of the same type', () => {
  const data = mapRealProjectsToWorkspace(
    [project({ propertyId: 'property-row' })],
    [item('anchor', { entity: { type: 'property', id: 'property-wbs' } })],
    {},
    [
      { id: 'doc-row', title: 'Row', state: 'ready', propertyId: 'property-row', createdAt: '2026-09-09T00:00:00.000Z' },
      { id: 'doc-wbs', title: 'Wbs', state: 'ready', propertyId: 'property-wbs', createdAt: '2026-09-09T00:00:00.000Z' },
    ],
  )

  const plan = planFor(data)
  assert.ok(plan)
  assert.deepEqual(plan.documents?.map((document) => document.id), ['doc-row'])
})
