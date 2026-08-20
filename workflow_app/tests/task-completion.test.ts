import { test } from 'node:test'
import assert from 'node:assert/strict'

import { completeWorkflowTaskCore } from '../task-completion'

// CRM-14O / Story 220 — durable regression coverage for the workflow
// task-completion seam (canonical task id -> engine task id -> engine
// completeTask). Proves the 1:1 correlation contract and that a canonical
// task without a workflow correlation can never advance the engine.

test('A. resolves canonical -> engine task and completes exactly once', async () => {
  const completed: string[] = []

  const result = await completeWorkflowTaskCore(
    { applicationTaskId: 'app-task-1', userId: 'u1', transitionName: 'done' },
    {
      findWorkflowTaskId: async (id) => (id === 'app-task-1' ? 'engine-task-1' : null),
      completeEngineTask: async (workflowTaskId, input) => {
        completed.push(workflowTaskId)
        assert.equal(input.userId, 'u1')
        assert.equal(input.transitionName, 'done')
      },
    },
  )

  assert.deepEqual(result, { workflowTaskId: 'engine-task-1' })
  assert.deepEqual(completed, ['engine-task-1'])
})

test('B. refuses to advance a canonical task with no workflow correlation', async () => {
  let advanced = false

  await assert.rejects(
    completeWorkflowTaskCore(
      { applicationTaskId: 'unrelated-task', userId: 'u1' },
      {
        findWorkflowTaskId: async () => null,
        completeEngineTask: async () => {
          advanced = true
        },
      },
    ),
    /No workflow task correlates to application task unrelated-task/,
  )

  assert.equal(advanced, false)
})

test('C. propagates the engine completion failure (no silent partial advance)', async () => {
  await assert.rejects(
    completeWorkflowTaskCore(
      { applicationTaskId: 'app-task-1', userId: 'u1' },
      {
        findWorkflowTaskId: async () => 'engine-task-1',
        completeEngineTask: async () => {
          throw new Error('Task cannot be completed in status: completed')
        },
      },
    ),
    /Task cannot be completed in status: completed/,
  )
})
