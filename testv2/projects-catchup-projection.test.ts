import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { Project } from '../services/project'
import type { WbsItem } from '../services/wbs'
import { dueDateKey, projectCatchUp } from '../ui/projects/catchup-projection'
import { mapCanonicalProjectWorkItems, mapRealProjectsToWorkspace } from '../ui/projects/service-projection'

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
    projectType: 'listing',
    playbookId: 'listing-onboarding',
    playbookVersion: 1,
    personId: null,
    propertyId: null,
    contractId: null,
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

test('Catch-Up canonical projection emits each persisted WBS item exactly once even when project has multiple perspective anchors', () => {
  const projects = [project()]
  const items = [
    item('parties', { category: 'clients', entity: { type: 'person', id: 'person-1' } }),
    item('property', { category: 'properties', entity: { type: 'property', id: 'property-1' } }),
    item('agreement', { category: 'contracts' }),
  ]

  const workspace = mapRealProjectsToWorkspace(projects, items)
  assert.equal(
    workspace.poles.filter((pole) => pole.projects.some((plan) => plan.id === 'project-1')).length,
    2,
    'the navigation perspective legitimately surfaces the same project under person and property poles',
  )

  const catchUp = mapCanonicalProjectWorkItems(projects, items)
  assert.equal(catchUp.length, 3)
  assert.equal(new Set(catchUp.map((entry) => entry.id)).size, 3)
  assert.deepEqual(catchUp.map((entry) => entry.id).sort(), ['agreement', 'parties', 'property'])
})

test('Catch-Up derives area from the canonical WBS row rather than a perspective copy', () => {
  const rows = mapCanonicalProjectWorkItems(
    [project()],
    [
      item('person-work', { category: 'clients', entity: { type: 'person', id: 'person-1' } }),
      item('property-work', { category: 'properties', entity: { type: 'property', id: 'property-1' } }),
      item('marketing-work', { category: 'marketing' }),
    ],
  )

  assert.equal(rows.find((row) => row.id === 'person-work')?.domain, 'people')
  assert.equal(rows.find((row) => row.id === 'property-work')?.domain, 'properties')
  assert.equal(rows.find((row) => row.id === 'marketing-work')?.domain, 'marketing')
})

test('Catch-Up treats persisted due dates as calendar dates without Puerto Rico timezone rollback', () => {
  assert.equal(dueDateKey('2026-09-14T00:00:00.000Z'), '2026-09-14')

  const rows = mapCanonicalProjectWorkItems(
    [project()],
    [item('due-today', { dueAt: '2026-09-14T00:00:00.000Z' })],
  )
  const buckets = projectCatchUp(rows, new Date(2026, 8, 14, 12, 0, 0))

  assert.deepEqual(buckets.today.map((entry) => entry.id), ['due-today'])
})

test('Catch-Up exposes real unfinished undated WBS as Unscheduled and never fabricates sample rows', () => {
  const rows = mapCanonicalProjectWorkItems(
    [project()],
    [
      item('open-undated'),
      item('doing-undated', { status: 'doing' }),
      item('done-undated', { status: 'done' }),
      item('dismissed-undated', { status: 'dismissed' }),
      item('future', { dueAt: '2026-09-20T12:00:00.000Z' }),
    ],
  )

  const buckets = projectCatchUp(rows, new Date(2026, 8, 14, 12, 0, 0))
  assert.deepEqual(buckets.today, [])
  assert.deepEqual(buckets.unscheduled.map((entry) => entry.id).sort(), ['doing-undated', 'open-undated'])
  assert.ok(buckets.unscheduled.every((entry) => !entry.id.startsWith('sample-')))
})
