import test from 'node:test'
import assert from 'node:assert/strict'

import {
  eventTypeToKind,
  systemToSystemId,
  outcomeToStatus,
  adaptRuntimeInspection,
  toTimelineEntries,
} from '../../lib/flight-recorder-adapter'
import { buildCausalGraph } from '../../lib/causal-graph'
import type { RuntimeInspection, TimelineEntry } from '../../lib/runtime-inspector'

// FLIGHT-RECORDER-ADAPTER — the pure bridge from real engine Runtime Inspection
// evidence into the Flight Recorder console read-model. Tests exercise the
// deterministic mapping (kind / system / status / causation) against the real
// engine's recorded vocabulary ('command' | 'domain' | 'workflow').

const T0 = '2026-08-28T14:00:00.000Z'

function entry(partial: Partial<TimelineEntry> & { id: string }): TimelineEntry {
  return {
    id: partial.id,
    occurredAt: partial.occurredAt ?? T0,
    relativeMs: partial.relativeMs ?? 0,
    eventType: partial.eventType ?? 'COMMAND_RECEIVED',
    system: partial.system ?? 'command',
    summary: partial.summary ?? null,
    outcome: partial.outcome ?? null,
    durationMs: partial.durationMs ?? null,
    nodeId: partial.nodeId ?? null,
    causationId: partial.causationId ?? null,
    commandId: partial.commandId ?? null,
    domainEventId: partial.domainEventId ?? null,
    metadata: partial.metadata ?? null,
    dealId: partial.dealId ?? null,
    propertyId: partial.propertyId ?? null,
    personId: partial.personId ?? null,
    transactionDocumentId: partial.transactionDocumentId ?? null,
    taskId: partial.taskId ?? null,
    signatureRequestId: partial.signatureRequestId ?? null,
    workflowDefinitionKey: partial.workflowDefinitionKey ?? null,
    workflowTransitionId: partial.workflowTransitionId ?? null,
  }
}

function inspection(events: TimelineEntry[], over: Partial<RuntimeInspection> = {}): RuntimeInspection {
  const sorted = [...events].sort((a, b) => a.occurredAt.localeCompare(b.occurredAt))
  return {
    workflowInstanceId: over.workflowInstanceId ?? 'wf-instance-1',
    definitionKey: over.definitionKey ?? 'purchase_transaction',
    definitionVersion: over.definitionVersion ?? 1,
    nodes: over.nodes ?? [],
    transitions: over.transitions ?? [],
    timeline: sorted,
    expectedVsActual: over.expectedVsActual ?? {
      nodesExpected: 0,
      nodesVisited: events.length,
      transitionsTaken: 0,
      unexpectedTransitions: 0,
      repeatedNodes: 0,
      repeatedTransitions: 0,
      failedEvents: 0,
      recoveredFailures: 0,
      currentNode: null,
      workflowElapsedMs: 0,
    },
    startIso: over.startIso ?? (sorted[0]?.occurredAt ?? null),
    endIso: over.endIso ?? (sorted[sorted.length - 1]?.occurredAt ?? null),
    currentNodeId: over.currentNodeId ?? null,
  }
}

test('eventTypeToKind maps the engine vocabulary onto console kinds', () => {
  assert.equal(eventTypeToKind('COMMAND_RECEIVED'), 'Command')
  assert.equal(eventTypeToKind('COMMAND_COMPLETED'), 'Command')
  assert.equal(eventTypeToKind('DOMAIN_EVENT_EMITTED'), 'DomainEvent')
  assert.equal(eventTypeToKind('WORKFLOW_STARTED'), 'Workflow')
  assert.equal(eventTypeToKind('NODE_ENTERED'), 'Workflow')
  assert.equal(eventTypeToKind('TRANSITION_TAKEN'), 'Workflow')
  assert.equal(eventTypeToKind('TASK_CREATED'), 'Task')
  assert.equal(eventTypeToKind('TIMER_FIRED'), 'Task')
  assert.equal(eventTypeToKind('SIGNATURE_SENT'), 'Integration')
  assert.equal(eventTypeToKind('DOCUMENT_CREATED'), 'Persistence')
  assert.equal(eventTypeToKind('FAILURE'), 'Workflow')
})

test('systemToSystemId maps the real engine systems and falls back by kind', () => {
  assert.equal(systemToSystemId('command', 'Command'), 'API Gateway')
  assert.equal(systemToSystemId('domain', 'DomainEvent'), 'Domain Model')
  assert.equal(systemToSystemId('workflow', 'Workflow'), 'Workflow Engine')
  assert.equal(systemToSystemId('signature', 'Integration'), 'BoldSign')
  assert.equal(systemToSystemId('postgres', 'Persistence'), 'PostgreSQL')
  // Unknown producer falls back from its kind (honest, stable).
  assert.equal(systemToSystemId('some_new_service', 'Persistence'), 'PostgreSQL')
  assert.equal(systemToSystemId('some_new_service', 'Command'), 'API Gateway')
})

test('outcomeToStatus surfaces failures and pending starts', () => {
  assert.equal(outcomeToStatus('SUCCESS', 'COMMAND_COMPLETED'), 'Success')
  assert.equal(outcomeToStatus('FAILURE', 'COMMAND_FAILED'), 'Failed')
  assert.equal(outcomeToStatus('STARTED', 'TASK_CREATED'), 'Pending')
  assert.equal(outcomeToStatus(null, 'NODE_ENTERED'), 'Pending')
  assert.equal(outcomeToStatus(null, 'WORKFLOW_COMPLETED'), 'Success')
  assert.equal(outcomeToStatus('RECOVERED', 'RECOVERED'), 'Success')
})

test('adaptRuntimeInspection builds a connected console trace from a command→domain→workflow chain', () => {
  const timeline = [
    entry({ id: 'c1', eventType: 'COMMAND_RECEIVED', commandId: 'cmd-1', summary: 'Command deal.create received', outcome: 'STARTED' }),
    entry({ id: 'c2', eventType: 'COMMAND_COMPLETED', commandId: 'cmd-1', summary: 'Command deal.create success', outcome: 'SUCCESS', occurredAt: '2026-08-28T14:00:01.000Z', relativeMs: 1000, causationId: 'c1' }),
    entry({ id: 'd1', eventType: 'DOMAIN_EVENT_EMITTED', domainEventId: 'evt-1', commandId: 'cmd-1', causationId: 'cmd-1', summary: 'Domain event DealCreated', system: 'domain', occurredAt: '2026-08-28T14:00:02.000Z', relativeMs: 2000, metadata: { dealId: 'DEAL-1', priority: 'High' } }),
    entry({ id: 'w1', eventType: 'WORKFLOW_STARTED', system: 'workflow', causationId: 'evt-1', occurredAt: '2026-08-28T14:00:03.000Z', relativeMs: 3000 }),
    entry({ id: 'w2', eventType: 'WORKFLOW_COMPLETED', system: 'workflow', causationId: 'w1', outcome: 'SUCCESS', occurredAt: '2026-08-28T14:00:04.000Z', relativeMs: 4000 }),
  ]
  const trace = adaptRuntimeInspection({
    inspection: inspection(timeline, { startIso: T0, endIso: '2026-08-28T14:00:04.000Z' }),
  })

  assert.equal(trace.summary.correlationId, 'wf-instance-1')
  assert.equal(trace.summary.status, 'Completed')
  assert.equal(trace.summary.eventCount, 5)
  assert.equal(trace.summary.systemCount, 3)
  assert.equal(trace.summary.rootTitle, 'Command deal.create received')
  assert.equal(trace.summary.rootKind, 'Command')

  const byId = new Map(trace.events.map((e) => [e.id, e]))
  const cmdDone = byId.get('c2')!
  assert.equal(cmdDone.kind, 'Command')
  assert.equal(cmdDone.system, 'API Gateway')
  assert.equal(cmdDone.status, 'Success')

  const domain = byId.get('d1')!
  assert.equal(domain.kind, 'DomainEvent')
  assert.equal(domain.system, 'Domain Model')
  // causation resolved through commandId -> the completed command event id.
  assert.equal(domain.causationId, 'c2')
  assert.deepEqual(domain.payload, { dealId: 'DEAL-1', priority: 'High' })
  assert.ok(domain.details.Summary)

  const wfStarted = byId.get('w1')!
  assert.equal(wfStarted.kind, 'Workflow')
  assert.equal(wfStarted.system, 'Workflow Engine')
  assert.equal(wfStarted.status, 'Pending')
  // causation 'evt-1' resolves to the domain event timeline id.
  assert.equal(wfStarted.causationId, 'd1')

  // The domain event lists its cause and its child.
  const titles = domain.relatedEventIds.map((r) => r.id)
  assert.ok(titles.includes('c2'))
  assert.ok(titles.includes('w1'))
})

test('adaptRuntimeInspection surfaces real business-context and related ids (no fabrication)', () => {
  const timeline = [
    entry({
      id: 'c1',
      eventType: 'COMMAND_RECEIVED',
      commandId: 'cmd-1',
      dealId: 'DEAL-2025-000123',
      propertyId: 'PROP-7',
      personId: 'PERSON-9',
      summary: 'Command deal.create received',
      outcome: 'STARTED',
    }),
    entry({
      id: 't1',
      eventType: 'TASK_CREATED',
      taskId: 'task-42',
      signatureRequestId: 'sig-77',
      dealId: 'DEAL-2025-000123',
      causationId: 'c1',
      occurredAt: '2026-08-28T14:00:01.000Z',
      relativeMs: 1000,
    }),
  ]
  const trace = adaptRuntimeInspection({ inspection: inspection(timeline) })

  // Business Context carries the REAL ids across the timeline.
  assert.equal(trace.summary.businessContext.dealId, 'DEAL-2025-000123')
  assert.equal(trace.summary.businessContext.property, 'PROP-7')
  assert.equal(trace.summary.businessContext.client, 'PERSON-9')
  assert.equal(trace.summary.businessContext.workflow, 'purchase_transaction')

  // Per-event details surface the recorded related ids.
  const task = trace.events.find((e) => e.id === 't1')!
  assert.equal(task.details.Task, 'task-42')
  assert.equal(task.details.Signature, 'sig-77')
  assert.equal(task.details.Deal, 'DEAL-2025-000123')
})

test('adaptRuntimeInspection prefers read-side resolved labels in the business context', () => {
  const timeline = [
    entry({ id: 'a', eventType: 'WORKFLOW_STARTED', dealId: 'deal-9', outcome: 'STARTED' }),
  ]
  const trace = adaptRuntimeInspection({
    inspection: inspection(timeline),
    resolvedBusinessContext: {
      dealId: 'deal-9',
      deal: 'Deal #9 — 123 Ocean View Dr',
      property: '123 Ocean View Dr',
      client: 'Maria & Juan Rodriguez',
    },
  })

  assert.equal(trace.summary.businessContext.deal, 'Deal #9 — 123 Ocean View Dr')
  assert.equal(trace.summary.businessContext.property, '123 Ocean View Dr')
  assert.equal(trace.summary.businessContext.client, 'Maria & Juan Rodriguez')
  // dealId stays the raw id for reference.
  assert.equal(trace.summary.businessContext.dealId, 'deal-9')
})

test('adaptRuntimeInspection is deterministic (same input → identical output)', () => {
  const timeline = [entry({ id: 'a', commandId: 'cmd-1', outcome: 'SUCCESS' })]
  const a = adaptRuntimeInspection({ inspection: inspection(timeline) })
  const b = adaptRuntimeInspection({ inspection: inspection(timeline) })
  assert.deepEqual(a, b)
})

test('unresolved causation becomes null and is surfaced as a Cause Ref detail', () => {
  const timeline = [
    entry({ id: 'a', commandId: 'cmd-1', summary: 'root', outcome: 'SUCCESS' }),
    entry({ id: 'b', commandId: 'cmd-2', causationId: 'missing-command', summary: 'orphan', occurredAt: '2026-08-28T14:00:01.000Z', relativeMs: 1000 }),
  ]
  const trace = adaptRuntimeInspection({ inspection: inspection(timeline) })
  const orphan = trace.events.find((e) => e.id === 'b')!
  assert.equal(orphan.causationId, null)
  assert.equal(orphan.details['Cause Ref'], 'missing-command')
})

test('a failed event flips the summary to Failed; a live trace stays InProgress', () => {
  const failed = adaptRuntimeInspection({
    inspection: inspection([entry({ id: 'a', eventType: 'COMMAND_FAILED', outcome: 'FAILURE' })]),
  })
  assert.equal(failed.summary.status, 'Failed')

  const live = adaptRuntimeInspection({
    inspection: inspection([entry({ id: 'a', eventType: 'NODE_ENTERED', outcome: 'STARTED' })]),
  })
  assert.equal(live.summary.status, 'InProgress')
})

test('toTimelineEntries round-trips adapted events back into a connected causal graph', () => {
  const timeline = [
    entry({ id: 'c1', eventType: 'COMMAND_RECEIVED', commandId: 'cmd-1', summary: 'Command deal.create received', outcome: 'STARTED' }),
    entry({ id: 'c2', eventType: 'COMMAND_COMPLETED', commandId: 'cmd-1', summary: 'Command deal.create success', outcome: 'SUCCESS', occurredAt: '2026-08-28T14:00:01.000Z', relativeMs: 1000, causationId: 'c1' }),
    entry({ id: 'd1', eventType: 'DOMAIN_EVENT_EMITTED', domainEventId: 'evt-1', commandId: 'cmd-1', causationId: 'cmd-1', summary: 'Domain event DealCreated', system: 'domain', occurredAt: '2026-08-28T14:00:02.000Z', relativeMs: 2000 }),
    entry({ id: 'w1', eventType: 'WORKFLOW_STARTED', system: 'workflow', causationId: 'evt-1', occurredAt: '2026-08-28T14:00:03.000Z', relativeMs: 3000 }),
    entry({ id: 'w2', eventType: 'WORKFLOW_COMPLETED', system: 'workflow', causationId: 'w1', outcome: 'SUCCESS', occurredAt: '2026-08-28T14:00:04.000Z', relativeMs: 4000 }),
  ]
  const trace = adaptRuntimeInspection({ inspection: inspection(timeline) })

  const reconstructed = toTimelineEntries(trace.events)
  // Original event identity is preserved (the console keeps the trace id).
  assert.deepEqual(
    reconstructed.map((t) => t.id).sort(),
    ['c1', 'c2', 'd1', 'w1', 'w2'].sort(),
  )
  // Causation was resolved to sibling ids during adaptation and survives.
  const d = reconstructed.find((t) => t.id === 'd1')!
  assert.equal(d.causationId, 'c2')
  const w = reconstructed.find((t) => t.id === 'w1')!
  assert.equal(w.causationId, 'd1')

  // The round-tripped timeline feeds the real causal projection and connects.
  const graph = buildCausalGraph(reconstructed)
  const byId = new Map(graph.nodes.map((n) => [n.id, n]))
  assert.ok(graph.edges.length > 0, 'reconstructed graph has causal edges')
  // Command stages collapse into one command node keyed by commandId; the domain
  // event keys by its own domainEventId — exactly like the Runtime Inspector.
  assert.ok(byId.has('cmd:cmd-1'), 'command node groups both stage events')
  assert.ok(byId.has('dom:evt-1'), 'domain event node keyed by its own id')
  const cmdNode = byId.get('cmd:cmd-1')!
  assert.equal(cmdNode.count, 2, 'both COMMAND_ stage events grouped')
})

