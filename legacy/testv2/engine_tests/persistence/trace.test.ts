import { test } from 'node:test'
import assert from 'node:assert/strict'

import { assertEngineSchema, PersistenceFixture } from './harness'
import type { WorkflowTraceRecord } from '../../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// V2 — Workflow Engine → Flight Recorder lifecycle instrumentation.
//
// The engine emits observer-only trace records (WORKFLOW_STARTED / NODE_ENTERED
// / TRANSITION_TAKEN / WORKFLOW_COMPLETED / WORKFLOW_FAILED) through the open
// step transaction. These are the evidence the Runtime Inspector overlays on
// the design-time topology. The recorder is optional and contained (absent =
// zero change; a throwing recorder must never gate the step).
// ---------------------------------------------------------------------------

// start -> end
const LINEAR = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

// start -> review (human task) -> end
const TASKED = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'review' }] },
    review: {
      id: 'review',
      type: 'task',
      name: 'Review',
      candidateGroups: ['u1'],
      transitions: [{ name: 'done', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  },
}

function capture() {
  const records: WorkflowTraceRecord[] = []
  return {
    records,
    recorder: (input: WorkflowTraceRecord) => {
      records.push(input)
    },
  }
}

test('engine records the full linear lifecycle into the flight recorder', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  const cap = capture()
  try {
    await f.seedDefinition('tunit_trace_linear', 1, LINEAR)
    const engine = f.makeEngine({ traceRecorder: cap.recorder })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_trace_linear',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })

    const types = cap.records.map((r) => r.eventType)
    assert.ok(types.includes('WORKFLOW_STARTED'), `expected WORKFLOW_STARTED, got ${types}`)
    assert.ok(types.includes('NODE_ENTERED'), `expected NODE_ENTERED, got ${types}`)
    assert.ok(types.includes('TRANSITION_TAKEN'), `expected TRANSITION_TAKEN, got ${types}`)
    assert.ok(types.includes('WORKFLOW_COMPLETED'), `expected WORKFLOW_COMPLETED, got ${types}`)

    const started = cap.records.find((r) => r.eventType === 'WORKFLOW_STARTED')!
    assert.equal(started.system, 'workflow')
    assert.equal(started.workflowInstanceId, processInstanceId)
    assert.equal(started.correlationId, processInstanceId)
    assert.equal(started.workflowDefinitionKey, 'tunit_trace_linear')

    // start -> end: both nodes entered, one transition taken.
    const entered = cap.records.filter((r) => r.eventType === 'NODE_ENTERED')
    assert.deepEqual(
      entered.map((r) => r.workflowNodeId).sort(),
      ['end', 'start'],
    )
    const moved = cap.records.find((r) => r.eventType === 'TRANSITION_TAKEN')!
    assert.equal(moved.workflowNodeId, 'start') // from
    assert.equal(moved.workflowTransitionId, 'end') // to
  } finally {
    await f.cleanup()
  }
})

test('engine records a human-gate node entry that waits, then completes', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  const cap = capture()
  try {
    await f.seedDefinition('tunit_trace_task', 1, TASKED)
    const engine = f.makeEngine({ traceRecorder: cap.recorder })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_trace_task',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })

    // After start, the process waits at the human task; it must NOT be terminal.
    assert.ok(
      cap.records.some((r) => r.eventType === 'NODE_ENTERED' && r.workflowNodeId === 'review'),
      'task node entered (human gate)',
    )
    assert.ok(!cap.records.some((r) => r.eventType === 'WORKFLOW_COMPLETED'))

    const tasks = await f.tasks(processInstanceId)
    assert.equal(tasks.length, 1)
    await engine.completeTask({ taskId: tasks[0].id, userId: 'u1', transitionName: 'done' })

    assert.ok(cap.records.some((r) => r.eventType === 'WORKFLOW_COMPLETED'))
    const completed = cap.records.find((r) => r.eventType === 'WORKFLOW_COMPLETED')!
    assert.equal(completed.outcome, 'completed')
  } finally {
    await f.cleanup()
  }
})

test('a throwing trace recorder never gates the engine step', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_trace_safe', 1, LINEAR)
    const engine = f.makeEngine({
      traceRecorder: () => {
        throw new Error('recorder exploded')
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_trace_safe',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed', 'engine completed despite recorder failure')
  } finally {
    await f.cleanup()
  }
})

test('engine threads the workflow subject into the trace business context', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  const cap = capture()
  try {
    await f.seedDefinition('tunit_trace_subject', 1, LINEAR)
    const engine = f.makeEngine({ traceRecorder: cap.recorder })
    await engine.startProcess({
      definitionKey: 'tunit_trace_subject',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
      subject: { subjectType: 'deal', subjectId: 'deal-777' },
    })

    const started = cap.records.find((r) => r.eventType === 'WORKFLOW_STARTED')!
    assert.equal(started.dealId, 'deal-777', 'deal-scoped subject carries dealId')
    assert.equal(started.propertyId, null, 'a deal subject does not name a property')
    assert.equal(started.personId, null, 'a deal subject does not name a person')

    const enteredStart = cap.records.find(
      (r) => r.eventType === 'NODE_ENTERED' && r.workflowNodeId === 'start',
    )!
    assert.equal(enteredStart.dealId, 'deal-777', 'node entry inherits the subject context')
  } finally {
    await f.cleanup()
  }
})
