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
