import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  buildRuntimeInspection,
  computeCurrentNode,
  eventsAtOrBefore,
  type RuntimeInspection,
} from '../../lib/runtime-inspector'
import type { TraceEvent } from '../../lib/workflow-trace'
import type { ProcessGraph } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// RUNTIME-INSPECTOR — overlay runtime trace evidence on the design-time graph.
// ---------------------------------------------------------------------------

const GRAPH: ProcessGraph = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'to_a', to: 'task_a' }] },
    task_a: { id: 'task_a', type: 'task', name: 'Task A', transitions: [{ name: 'to_b', to: 'task_b' }] },
    task_b: { id: 'task_b', type: 'task', name: 'Task B', transitions: [{ name: 'to_end', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

function tr(o: Partial<TraceEvent> & { eventType: string; occurredAt: string }): TraceEvent {
  return {
    eventType: o.eventType,
    system: o.system ?? 'test',
    occurredAt: o.occurredAt,
    completedAt: o.completedAt ?? null,
    durationMs: o.durationMs ?? null,
    outcome: o.outcome ?? null,
    traceId: o.traceId ?? null,
    correlationId: o.correlationId ?? null,
    causationId: o.causationId ?? null,
    dealId: o.dealId ?? null,
    personId: o.personId ?? null,
    propertyId: o.propertyId ?? null,
    transactionDocumentId: o.transactionDocumentId ?? null,
    workflowInstanceId: o.workflowInstanceId ?? 'wf-1',
    workflowDefinitionKey: o.workflowDefinitionKey ?? null,
    workflowDefinitionVersion: o.workflowDefinitionVersion ?? null,
    workflowNodeId: o.workflowNodeId ?? null,
    workflowTransitionId: o.workflowTransitionId ?? null,
    commandId: o.commandId ?? null,
    domainEventId: o.domainEventId ?? null,
    taskId: o.taskId ?? null,
    timerJobId: o.timerJobId ?? null,
    signatureRequestId: o.signatureRequestId ?? null,
    externalReference: o.externalReference ?? null,
    summary: o.summary ?? null,
    metadata: o.metadata ?? null,
    sourceSystem: o.sourceSystem ?? null,
    sourceEventId: o.sourceEventId ?? null,
  }
}

function completedJourney(): TraceEvent[] {
  return [
    tr({ eventType: 'WORKFLOW_STARTED', occurredAt: '2026-08-28T14:00:00.000Z' }),
    tr({ eventType: 'NODE_ENTERED', occurredAt: '2026-08-28T14:00:01.000Z', workflowNodeId: 'start' }),
    tr({ eventType: 'NODE_COMPLETED', occurredAt: '2026-08-28T14:00:02.000Z', workflowNodeId: 'start', outcome: 'SUCCESS' }),
    tr({ eventType: 'TRANSITION_TAKEN', occurredAt: '2026-08-28T14:00:03.000Z', workflowNodeId: 'start', workflowTransitionId: 'task_a' }),
    tr({ eventType: 'NODE_ENTERED', occurredAt: '2026-08-28T14:00:04.000Z', workflowNodeId: 'task_a' }),
    tr({ eventType: 'NODE_COMPLETED', occurredAt: '2026-08-28T14:00:05.000Z', workflowNodeId: 'task_a', outcome: 'SUCCESS' }),
    tr({ eventType: 'TRANSITION_TAKEN', occurredAt: '2026-08-28T14:00:06.000Z', workflowNodeId: 'task_a', workflowTransitionId: 'task_b' }),
    tr({ eventType: 'NODE_ENTERED', occurredAt: '2026-08-28T14:00:07.000Z', workflowNodeId: 'task_b' }),
    tr({ eventType: 'NODE_COMPLETED', occurredAt: '2026-08-28T14:00:08.000Z', workflowNodeId: 'task_b', outcome: 'SUCCESS' }),
    tr({ eventType: 'TRANSITION_TAKEN', occurredAt: '2026-08-28T14:00:09.000Z', workflowNodeId: 'task_b', workflowTransitionId: 'end' }),
    tr({ eventType: 'NODE_ENTERED', occurredAt: '2026-08-28T14:00:10.000Z', workflowNodeId: 'end' }),
    tr({ eventType: 'WORKFLOW_COMPLETED', occurredAt: '2026-08-28T14:00:11.000Z' }),
  ]
}

test('completed journey: current node is null, all nodes completed', () => {
  const events = completedJourney()
  assert.equal(computeCurrentNode(events), null)
  const ins = buildRuntimeInspection('wf-1', GRAPH, events)
  const byId = Object.fromEntries(ins.nodes.map((n) => [n.nodeId, n.state]))
  assert.equal(byId['start'], 'COMPLETED')
  assert.equal(byId['task_a'], 'COMPLETED')
  assert.equal(byId['task_b'], 'COMPLETED')
  assert.equal(byId['end'], 'COMPLETED')
  assert.equal(ins.currentNodeId, null)
  assert.equal(ins.expectedVsActual.unexpectedTransitions, 0)
  assert.equal(ins.expectedVsActual.repeatedNodes, 0)
})

test('current node is the latest entered-but-not-completed node', () => {
  const events = completedJourney().slice(0, 5) // ends at task_a entered (not completed)
  assert.equal(computeCurrentNode(events), 'task_a')
  const ins = buildRuntimeInspection('wf-1', GRAPH, events)
  assert.equal(ins.currentNodeId, 'task_a')
  const byId = Object.fromEntries(ins.nodes.map((n) => [n.nodeId, n.state]))
  assert.equal(byId['task_a'], 'CURRENT')
  assert.equal(byId['task_b'], 'NOT_VISITED')
})

test('time machine: at timestamp T, future nodes are not visited', () => {
  const events = completedJourney()
  const beforeTaskB = buildRuntimeInspection('wf-1', GRAPH, events, '2026-08-28T14:00:06.500Z')
  const byId = Object.fromEntries(beforeTaskB.nodes.map((n) => [n.nodeId, n.state]))
  assert.equal(byId['task_a'], 'COMPLETED')
  assert.equal(byId['task_b'], 'NOT_VISITED') // not reached before T
  assert.equal(byId['end'], 'NOT_VISITED')
})

test('eventsAtOrBefore filters to the timestamp', () => {
  const events = completedJourney()
  const at = eventsAtOrBefore(events, '2026-08-28T14:00:04.000Z')
  assert.ok(at.every((e) => new Date(e.occurredAt).getTime() <= new Date('2026-08-28T14:00:04.000Z').getTime()))
  assert.equal(at[at.length - 1].eventType, 'NODE_ENTERED')
})

test('repeated node and repeated transition are counted, not suppressed', () => {
  const events = [
    ...completedJourney(),
    tr({ eventType: 'NODE_ENTERED', occurredAt: '2026-08-28T14:01:00.000Z', workflowNodeId: 'task_b' }),
    tr({ eventType: 'NODE_COMPLETED', occurredAt: '2026-08-28T14:01:01.000Z', workflowNodeId: 'task_b', outcome: 'SUCCESS' }),
    tr({ eventType: 'TRANSITION_TAKEN', occurredAt: '2026-08-28T14:01:02.000Z', workflowNodeId: 'task_b', workflowTransitionId: 'end' }),
  ]
  const ins = buildRuntimeInspection('wf-1', GRAPH, events)
  const taskB = ins.nodes.find((n) => n.nodeId === 'task_b')!
  assert.equal(taskB.executionCount, 2)
  assert.equal(ins.expectedVsActual.repeatedNodes, 1)
  const tbToEnd = ins.transitions.find((t) => t.fromNodeId === 'task_b' && t.toNodeId === 'end')
  assert.equal(tbToEnd?.traversedCount, 2)
  assert.equal(ins.expectedVsActual.repeatedTransitions, 1)
})

test('unexpected transition is flagged; unused legal branch is not', () => {
  const events = [
    tr({ eventType: 'WORKFLOW_STARTED', occurredAt: '2026-08-28T14:00:00.000Z' }),
    tr({ eventType: 'NODE_ENTERED', occurredAt: '2026-08-28T14:00:01.000Z', workflowNodeId: 'start' }),
    tr({ eventType: 'TRANSITION_TAKEN', occurredAt: '2026-08-28T14:00:02.000Z', workflowNodeId: 'start', workflowTransitionId: 'NOT_A_REAL_NODE' }),
  ]
  const ins = buildRuntimeInspection('wf-1', GRAPH, events)
  assert.equal(ins.expectedVsActual.unexpectedTransitions, 1)
  // The unused legal branch task_a->task_b is not an anomaly.
  assert.equal(ins.expectedVsActual.transitionsTaken, 1)
})

test('failed then recovered node shows recovery evidence', () => {
  const events = [
    tr({ eventType: 'NODE_ENTERED', occurredAt: '2026-08-28T14:00:01.000Z', workflowNodeId: 'task_a' }),
    tr({ eventType: 'FAILURE', occurredAt: '2026-08-28T14:00:02.000Z', workflowNodeId: 'task_a', outcome: 'FAILURE' }),
    tr({ eventType: 'RETRY', occurredAt: '2026-08-28T14:00:03.000Z', workflowNodeId: 'task_a' }),
    tr({ eventType: 'RECOVERED', occurredAt: '2026-08-28T14:00:04.000Z', workflowNodeId: 'task_a', outcome: 'RECOVERED' }),
    tr({ eventType: 'NODE_COMPLETED', occurredAt: '2026-08-28T14:00:05.000Z', workflowNodeId: 'task_a', outcome: 'SUCCESS' }),
  ]
  const ins = buildRuntimeInspection('wf-1', GRAPH, events)
  assert.equal(ins.expectedVsActual.failedEvents, 1)
  assert.equal(ins.expectedVsActual.recoveredFailures, 1)
})

test('mapper without runtime evidence still works (all NOT_VISITED)', () => {
  const ins: RuntimeInspection = buildRuntimeInspection('wf-1', GRAPH, [])
  assert.equal(ins.nodes.length, Object.keys(GRAPH.nodes).length)
  assert.ok(ins.nodes.every((n) => n.state === 'NOT_VISITED'))
  assert.equal(ins.currentNodeId, null)
  assert.equal(ins.timeline.length, 0)
  assert.equal(ins.expectedVsActual.nodesVisited, 0)
})

test('timeline is chronological with relative elapsed time', () => {
  const ins = buildRuntimeInspection('wf-1', GRAPH, completedJourney())
  const times = ins.timeline.map((t) => new Date(t.occurredAt).getTime())
  for (let i = 1; i < times.length; i++) assert.ok(times[i] >= times[i - 1])
  assert.equal(ins.timeline[0].relativeMs, 0)
  assert.ok(ins.timeline[1].relativeMs > 0)
  assert.equal(ins.timeline[0].eventType, 'WORKFLOW_STARTED')
})

