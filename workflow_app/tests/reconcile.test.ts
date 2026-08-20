import { test } from 'node:test'
import assert from 'node:assert/strict'
import { reconcileWorkflowsCore } from '../reconcile'
import { reconcileTaskMaterializationCore, type EngineTaskView } from '../task-reconciliation'

test('reconcileWorkflowsCore aggregates start + materialize results', async () => {
  const report = await reconcileWorkflowsCore({
    startMissing: async () => ['a', 'b'],
    materializeTasks: async () => ({ materialized: 3, skipped: 1 }),
  })
  assert.deepEqual(report, { startedInstances: 2, materializedTasks: 3, skippedTasks: 1 })
})

test('task materialization reconciliation creates each task once', async () => {
  const tasks: EngineTaskView[] = [
    { workflowTaskId: 'wt-1', title: 'Inspection', subjectType: 'deal', subjectId: 'deal-1', dealId: 'deal-1' },
    { workflowTaskId: 'wt-2', title: 'Title', subjectType: 'deal', subjectId: 'deal-1', dealId: 'deal-1' },
  ]
  const seen = new Set<string>()
  const res = await reconcileTaskMaterializationCore(tasks, async (t) => {
    if (seen.has(t.workflowTaskId)) return { applicationTaskId: 'x', created: false }
    seen.add(t.workflowTaskId)
    return { applicationTaskId: `task-${seen.size}`, created: true }
  })
  assert.equal(res.materialized, 2)
  assert.equal(res.skipped, 0)

  // Repeat: everything already correlated -> skipped, nothing created.
  const again = await reconcileTaskMaterializationCore(tasks, async (t) => {
    return { applicationTaskId: 'x', created: false }
  })
  assert.equal(again.materialized, 0)
  assert.equal(again.skipped, 2)
})
