import { test } from 'node:test'
import assert from 'node:assert/strict'
import { materializeEngineTaskCore, type MaterializeDeps } from '../task-materialization'

function makeDeps(initial: Record<string, string> = {}) {
  const correlations = new Map<string, string>(Object.entries(initial))
  const created: string[] = []
  const deps: MaterializeDeps = {
    findCorrelation: async (wid) => correlations.get(wid) ?? null,
    createTask: async (i) => {
      created.push(i.title)
      return { id: `task-${created.length}` }
    },
    correlate: async (wid, appId) => {
      correlations.set(wid, appId)
    },
  }
  return { deps, correlations, created }
}

test('first materialization creates and correlates exactly once', async () => {
  const { deps, correlations, created } = makeDeps()

  const res = await materializeEngineTaskCore(
    { workflowTaskId: 'wt-1', title: 'Inspection', subjectType: 'deal', subjectId: 'deal-1' },
    deps,
  )

  assert.deepEqual(res, { applicationTaskId: 'task-1', created: true })
  assert.equal(created.length, 1)
  assert.equal(correlations.get('wt-1'), 'task-1')
})

test('duplicate/retry returns the existing correlation without creating a task', async () => {
  const { deps, created } = makeDeps({ 'wt-1': 'task-9' })

  const res = await materializeEngineTaskCore(
    { workflowTaskId: 'wt-1', title: 'Inspection', subjectType: 'deal', subjectId: 'deal-1' },
    deps,
  )

  assert.deepEqual(res, { applicationTaskId: 'task-9', created: false })
  assert.equal(created.length, 0)
})

test('missing correlation creates a new canonical task (stale correlation is detected by task status, not here)', async () => {
  const { deps, created } = makeDeps()

  const res = await materializeEngineTaskCore(
    { workflowTaskId: 'wt-2', title: 'Title', subjectType: 'deal', subjectId: 'deal-1' },
    deps,
  )

  assert.equal(res.created, true)
  assert.equal(created.length, 1)
})
