import test from 'node:test'
import assert from 'node:assert/strict'

import {
  buildFlightRecorderWorkflow,
  mapEventToWorkflowNode,
  toFlightRecorderEvent,
  QA_GOLDEN_DEAL_MARKER,
} from '../flight-recorder-read'
import type { FlightRecorderWorkflow } from '../flight-recorder-read'
import type { ProcessGraph } from '../../workflow_engine/lib/workflow/types'
import type { TraceEvent } from '../../lib/workflow-trace'

// ---------------------------------------------------------------------------
// FLIGHT-RECORDER-READ — the canonical transaction read model.
//
// Tests prove the work-order's core guarantees without a database: exact
// definition usage, immutable-id node mapping, supporting events preserved,
// all definition nodes represented with derived execution state, no fake events,
// multiple workflows, unresolved nodes, and missing-trace behavior.
// ---------------------------------------------------------------------------

const GRAPH: ProcessGraph = {
  startNodeId: 'start',
  nodes: {
    start: {
      id: 'start',
      type: 'start',
      name: 'Start',
      transitions: [{ name: 'go', to: 'task_a' }],
    },
    task_a: {
      id: 'task_a',
      type: 'task',
      name: 'Contract / P&S',
      transitions: [{ name: 'done', to: 'end' }],
    },
    task_b: {
      id: 'task_b',
      type: 'task',
      name: 'Inspections',
      transitions: [{ name: 'done', to: 'end' }],
    },
    end: { id: 'end', type: 'end', name: 'End' },
  },
}

function ev(partial: Partial<TraceEvent> & { eventType: TraceEvent['eventType'] }): TraceEvent {
  return {
    id: partial.id ?? null,
    eventType: partial.eventType,
    system: partial.system ?? 'workflow',
    occurredAt: partial.occurredAt ?? '2026-08-01T00:00:00.000Z',
    completedAt: partial.completedAt ?? null,
    durationMs: partial.durationMs ?? null,
    outcome: partial.outcome ?? null,
    traceId: partial.traceId ?? null,
    correlationId: partial.correlationId ?? 'corr-1',
    causationId: partial.causationId ?? null,
    dealId: partial.dealId ?? null,
    personId: partial.personId ?? null,
    propertyId: partial.propertyId ?? null,
    transactionDocumentId: partial.transactionDocumentId ?? null,
    workflowInstanceId: partial.workflowInstanceId ?? 'wf-1',
    workflowDefinitionKey: partial.workflowDefinitionKey ?? 'purchase_transaction',
    workflowDefinitionVersion: partial.workflowDefinitionVersion ?? 1,
    workflowNodeId: partial.workflowNodeId ?? null,
    workflowTransitionId: partial.workflowTransitionId ?? null,
    commandId: partial.commandId ?? null,
    domainEventId: partial.domainEventId ?? null,
    taskId: partial.taskId ?? null,
    timerJobId: partial.timerJobId ?? null,
    signatureRequestId: partial.signatureRequestId ?? null,
    externalReference: partial.externalReference ?? null,
    summary: partial.summary ?? null,
    metadata: partial.metadata ?? null,
    sourceSystem: partial.sourceSystem ?? null,
    sourceEventId: partial.sourceEventId ?? null,
  }
}

function build(events: TraceEvent[]): FlightRecorderWorkflow {
  return buildFlightRecorderWorkflow({
    workflowInstanceId: 'wf-1',
    definitionId: 'def-1',
    definitionKey: 'purchase_transaction',
    definitionVersion: 1,
    definitionMissing: false,
    status: 'active',
    graph: GRAPH,
    events,
  }).workflow
}

test('FLIGHT-RECORDER-READ TEST 4/7: all definition nodes are represented, future nodes NOT_VISITED', () => {
  const events = [
    ev({ eventType: 'NODE_ENTERED', workflowNodeId: 'start' }),
    ev({ eventType: 'NODE_COMPLETED', workflowNodeId: 'start', outcome: 'SUCCESS', occurredAt: '2026-08-01T00:00:01.000Z' }),
    ev({ eventType: 'NODE_ENTERED', workflowNodeId: 'task_a', occurredAt: '2026-08-01T00:00:02.000Z' }),
  ]
  const wf = build(events)
  const ids = Object.keys(wf.nodeStates).sort()
  assert.deepEqual(ids, ['end', 'start', 'task_a', 'task_b'], 'every definition node is present')
  assert.equal(wf.nodeStates['start'].state, 'COMPLETED', 'visited+completed node')
  assert.equal(wf.nodeStates['task_a'].state, 'CURRENT', 'entered-but-not-completed node is current')
  assert.equal(wf.nodeStates['task_b'].state, 'NOT_VISITED', 'future node stays unvisited')
  assert.equal(wf.nodeStates['end'].state, 'NOT_VISITED')
})

test('FLIGHT-RECORDER-READ TEST 2/13: node mapping uses the exact definition by immutable id', () => {
  const mapped = toFlightRecorderEvent(
    ev({ eventType: 'SIGNATURE_SENT', workflowNodeId: 'task_a', sourceEventId: 'e-1', id: 'e-1' }),
    GRAPH,
  )
  assert.equal(mapped.workflowNodeId, 'task_a')
  assert.deepEqual(mapped.mappedWorkflowNode, {
    id: 'task_a',
    name: 'Contract / P&S',
    type: 'task',
    description: null,
  })

  const unknown = toFlightRecorderEvent(
    ev({ eventType: 'SIGNATURE_SENT', workflowNodeId: 'ghost_node', id: 'e-2' }),
    GRAPH,
  )
  assert.equal(unknown.mappedWorkflowNode, null, 'unresolved node mapping')
  assert.equal(unknown.workflowNodeId, 'ghost_node', 'event preserved with its real node id')
})
test('FLIGHT-RECORDER-READ TEST 3: supporting events without a node stay in the model', () => {
  const supporting = ev({ eventType: 'DOMAIN_EVENT_EMITTED', domainEventId: 'evt-9', id: 'e-3' })
  const events = [ev({ eventType: 'NODE_ENTERED', workflowNodeId: 'start' }), supporting]
  const { workflow, events: out } = buildFlightRecorderWorkflow({
    workflowInstanceId: 'wf-1',
    definitionId: 'def-1',
    definitionKey: 'purchase_transaction',
    definitionVersion: 1,
    definitionMissing: false,
    status: 'active',
    graph: GRAPH,
    events,
  })
  assert.equal(workflow.nodeStates['start'].state, 'CURRENT')
  assert.equal(out.length, 2, 'supporting event kept')
  assert.equal(out[1].workflowNodeId, null)
  assert.equal(out[1].domainEventId, 'evt-9')
})

test('FLIGHT-RECORDER-READ TEST 11: definition nodes never become timeline events', () => {
  const events = [ev({ eventType: 'NODE_ENTERED', workflowNodeId: 'start', id: 'e-4' })]
  const { events: out } = buildFlightRecorderWorkflow({
    workflowInstanceId: 'wf-1',
    definitionId: 'def-1',
    definitionKey: 'purchase_transaction',
    definitionVersion: 1,
    definitionMissing: false,
    status: 'active',
    graph: GRAPH,
    events,
  })
  assert.equal(out.length, 1, 'only real trace events appear')
  assert.equal(out[0].eventId, 'e-4')
})

test('FLIGHT-RECORDER-READ TEST 14: missing trace still renders the master workflow', () => {
  const wf = build([])
  assert.equal(wf.currentNodeId, null)
  assert.deepEqual(Object.keys(wf.nodeStates).sort(), ['end', 'start', 'task_a', 'task_b'])
  for (const s of Object.values(wf.nodeStates)) assert.equal(s.state, 'NOT_VISITED')
})

test('FLIGHT-RECORDER-READ TEST 12: multiple workflows keep events and nodes on their own instance', () => {
  const wfA = buildFlightRecorderWorkflow({
    workflowInstanceId: 'wf-A',
    definitionId: 'def-a',
    definitionKey: 'purchase_transaction',
    definitionVersion: 1,
    definitionMissing: false,
    status: 'completed',
    graph: GRAPH,
    events: [ev({ eventType: 'NODE_ENTERED', workflowNodeId: 'start', workflowInstanceId: 'wf-A', id: 'a1' })],
  })
  const wfB = buildFlightRecorderWorkflow({
    workflowInstanceId: 'wf-B',
    definitionId: 'def-b',
    definitionKey: 'amendment',
    definitionVersion: 1,
    definitionMissing: false,
    status: 'active',
    graph: GRAPH,
    events: [ev({ eventType: 'NODE_ENTERED', workflowNodeId: 'task_b', workflowInstanceId: 'wf-B', id: 'b1' })],
  })
  assert.equal(wfA.events[0].workflowInstanceId, 'wf-A')
  assert.equal(wfA.events[0].mappedWorkflowNode?.id, 'start')
  assert.equal(wfB.events[0].workflowInstanceId, 'wf-B')
  assert.equal(wfB.events[0].mappedWorkflowNode?.id, 'task_b')
  assert.equal(wfA.workflow.definitionKey, 'purchase_transaction')
  assert.equal(wfB.workflow.definitionKey, 'amendment')
})

test('FLIGHT-RECORDER-READ TEST 1/8: historical fidelity — the passed exact definition is used', () => {
  const older: ProcessGraph = {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', name: 'Start', transitions: [{ name: 'go', to: 'legacy' }] },
      legacy: { id: 'legacy', type: 'task', name: 'Legacy Step', transitions: [{ name: 'done', to: 'end' }] },
      end: { id: 'end', type: 'end', name: 'End' },
    },
  }
  const wf = buildFlightRecorderWorkflow({
    workflowInstanceId: 'wf-hist',
    definitionId: 'def-v231',
    definitionKey: 'purchase_transaction',
    definitionVersion: 3,
    definitionMissing: false,
    status: 'completed',
    graph: older,
    events: [],
  }).workflow
  assert.equal(wf.definitionVersion, 3)
  assert.ok(wf.nodeStates['legacy'], 'v2.3.1 node present')
  assert.ok(!wf.nodeStates['task_a'], 'newer v2.4 node absent from a historical execution')
})

test('FLIGHT-RECORDER-READ QA golden marker is a stable, deterministic identity', () => {
  assert.equal(QA_GOLDEN_DEAL_MARKER, 'qa-flight-recorder-golden')
  assert.ok(QA_GOLDEN_DEAL_MARKER.length > 0)
  // Idempotency anchor: the marker is the business-key predicate used to find
  // and reset ONLY the QA fixture, never general DEV data.
  assert.ok(!QA_GOLDEN_DEAL_MARKER.includes("'"))
})


