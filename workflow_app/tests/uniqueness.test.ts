import { test } from 'node:test'
import assert from 'node:assert/strict'
import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine'
import { FakeSql } from '../../workflow_engine/tests/fake-sql'
import { stubEvaluator } from '../../workflow_engine/tests/fixtures'
import { startWorkflowCore } from '../start-core'

const LINEAR_A = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'waitA' }] },
    waitA: { id: 'waitA', type: 'task', name: 'Wait A', transitions: [{ name: 'done', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

const LINEAR_B = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'waitB' }] },
    waitB: { id: 'waitB', type: 'task', name: 'Wait B', transitions: [{ name: 'done', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

test('B. two different workflow definitions may be active for the same deal', async () => {
  const fake = new FakeSql()
  fake.seedDefinition('workflow-a', 1, LINEAR_A)
  fake.seedDefinition('workflow-b', 1, LINEAR_B)
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator })

  await engine.startProcess({
    definitionKey: 'workflow-a',
    startedBy: 'x',
    subject: { subjectType: 'deal', subjectId: 'deal-123' },
  })
  await engine.startProcess({
    definitionKey: 'workflow-b',
    startedBy: 'x',
    subject: { subjectType: 'deal', subjectId: 'deal-123' },
  })

  const active = fake.store.processInstances.filter(
    (pi) => pi.subject_type === 'deal' && pi.subject_id === 'deal-123' && pi.status === 'active',
  )
  assert.equal(active.length, 2, 'a different definition may be active for the same deal')
})

test('A. startWorkflowCore is idempotent when an active instance exists', async () => {
  const res = await startWorkflowCore('deal-1', {
    findActive: async () => 'inst-1',
    readFacts: async () => null,
    start: async () => 'never',
  })
  assert.deepEqual(res, { instanceId: 'inst-1', started: false })
})

test('A. concurrent duplicate start (unique violation) resolves to the winner', async () => {
  let calls = 0
  const res = await startWorkflowCore('deal-1', {
    findActive: async () => (calls++ === 0 ? null : 'inst-winner'),
    readFacts: async () => ({ financingApplicable: false }),
    start: async () => {
      throw Object.assign(new Error('duplicate'), { code: '23505' })
    },
  })
  assert.deepEqual(res, { instanceId: 'inst-winner', started: false })
})
