import { test } from 'node:test'
import assert from 'node:assert/strict'

import { mapProjectToTimeline } from '../ui/projects/timeline-projection'
import type { ProjectPlan, ProjectWorkNode } from '../ui/projects/model'

const node = (
  id: string,
  title: string,
  overrides: Partial<ProjectWorkNode> = {},
): ProjectWorkNode => ({
  id,
  title,
  type: 'task',
  status: 'not-started',
  ...overrides,
})

const plan = (nodes: ProjectWorkNode[], overrides: Partial<ProjectPlan> = {}): ProjectPlan => ({
  id: 'p1',
  title: 'Sea to Soul Listing',
  kind: 'LISTING',
  status: 'active',
  progress: 42,
  phaseLabel: 'Agreement',
  workNodes: nodes,
  ...overrides,
})

/** Local calendar date as y-m-d, so assertions do not depend on the runner's timezone. */
const ymd = (value: Date | undefined): string => {
  assert.ok(value instanceof Date, 'expected a Date')
  return `${value.getFullYear()}-${String(value.getMonth() + 1).padStart(2, '0')}-${String(value.getDate()).padStart(2, '0')}`
}

test('a real due date lands on its own calendar date, not the day before (UTC-4 trap)', () => {
  // WBS due dates persist as UTC midnight. Reading local parts of that instant
  // would put every deadline a day early in Puerto Rico.
  const { tasks, synthetic } = mapProjectToTimeline(
    plan([node('a', 'Listing Agreement', { dueAt: '2026-09-10T00:00:00.000Z' })]),
  )
  const task = tasks.find((t) => t.text === 'Listing Agreement')
  assert.ok(task)
  assert.equal(synthetic, false)
  assert.equal(ymd(task.end), '2026-09-10')
})

test('the project is the root summary and carries its REAL progress', () => {
  const { tasks } = mapProjectToTimeline(plan([node('a', 'Parties')], { progress: 42 }))
  const root = tasks[0]
  assert.equal(root.type, 'summary')
  assert.equal(root.parent, 0)
  assert.equal(root.text, 'Sea to Soul Listing')
  assert.equal(root.progress, 42)
})

test('hierarchy is preserved: nodes hang off the project, children off their node', () => {
  const { tasks } = mapProjectToTimeline(
    plan([
      node('agreement', 'Listing Agreement', {
        children: [node('sig', 'Seller Signature')],
      }),
      node('media', 'Cabinet + Photos'),
    ]),
  )
  const agreement = tasks.find((t) => t.text === 'Listing Agreement')
  const signature = tasks.find((t) => t.text === 'Seller Signature')
  const media = tasks.find((t) => t.text === 'Cabinet + Photos')
  assert.ok(agreement && signature && media)
  // A node with children is a summary; its child points at it, not at the project.
  assert.equal(agreement.type, 'summary')
  assert.equal(agreement.parent, 1)
  assert.equal(signature.parent, agreement.id)
  assert.equal(media.type, 'task')
  assert.equal(media.parent, 1)
})

test('work-node status maps to Gantt progress', () => {
  const { tasks } = mapProjectToTimeline(
    plan([
      node('done', 'Done thing', { status: 'complete' }),
      node('doing', 'Doing thing', { status: 'in-progress' }),
      node('waiting', 'Waiting thing', { status: 'waiting' }),
      node('open', 'Open thing', { status: 'not-started' }),
    ]),
  )
  const progress = (text: string) => tasks.find((t) => t.text === text)?.progress
  assert.equal(progress('Done thing'), 100)
  assert.equal(progress('Doing thing'), 60)
  assert.equal(progress('Waiting thing'), 40)
  assert.equal(progress('Open thing'), 0)
})

test('an undated project is flagged synthetic and sequenced by plan order', () => {
  const { tasks, links, synthetic } = mapProjectToTimeline(
    plan([node('a', 'First'), node('b', 'Second'), node('c', 'Third')]),
  )
  assert.equal(synthetic, true)
  const first = tasks.find((t) => t.text === 'First')
  const second = tasks.find((t) => t.text === 'Second')
  const third = tasks.find((t) => t.text === 'Third')
  assert.ok(first && second && third)
  // Sequential, non-overlapping sample slots.
  assert.ok(first.start! < second.start!)
  assert.ok(second.start! < third.start!)
  assert.equal(first.duration, 3)
  // Sample sequencing produces links; a real project does not get invented ones.
  assert.equal(links.length, 2)
  assert.equal(links[0].source, first.id)
  assert.equal(links[0].target, second.id)
  assert.equal(links[0].type, 'e2s')
})

test('invented dependency links are emitted ONLY for a sample schedule', () => {
  const { links, synthetic } = mapProjectToTimeline(
    plan([
      node('a', 'First', { dueAt: '2026-09-10T00:00:00.000Z' }),
      node('b', 'Second', { dueAt: '2026-09-14T00:00:00.000Z' }),
    ]),
  )
  assert.equal(synthetic, false)
  assert.deepEqual(links, [])
})

test('a project with no work nodes yields an empty timeline, not a fabricated one', () => {
  const { tasks, links, synthetic } = mapProjectToTimeline(plan([]))
  assert.deepEqual(tasks, [])
  assert.deepEqual(links, [])
  assert.equal(synthetic, false)
})

test('the projection is deterministic (no wall-clock input)', () => {
  const project = plan([node('a', 'First'), node('b', 'Second')])
  const before = mapProjectToTimeline(project)
  const after = mapProjectToTimeline(project)
  assert.deepEqual(before.tasks, after.tasks)
  assert.deepEqual(before.links, after.links)
})
