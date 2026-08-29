import test from 'node:test'
import assert from 'node:assert/strict'

import {
  buildCausalEventPairs,
  buildUnresolvedCauses,
  buildSelectionCausality,
  groupEventsBySystem,
  rawEventFields,
} from '../../lib/flight-recorder-views'
import type { TraceEvent } from '../../lib/flight-recorder-adapter'

// ---------------------------------------------------------------------------
// FLIGHT-RECORDER-VIEWS — pure projections shared by Causality / Swimlane / Raw.
// ---------------------------------------------------------------------------

function ev(partial: Partial<TraceEvent> & { id: string }): TraceEvent {
  return {
    id: partial.id,
    correlationId: partial.correlationId ?? 'corr-1',
    causationId: partial.causationId ?? null,
    commandId: partial.commandId ?? null,
    domainEventId: partial.domainEventId ?? null,
    workflowNodeId: partial.workflowNodeId ?? null,
    kind: partial.kind ?? 'Workflow',
    type: partial.type ?? 'WORKFLOW_STARTED',
    title: partial.title ?? partial.id,
    subtitle: partial.subtitle ?? '',
    system: partial.system ?? 'workflow',
    status: partial.status ?? 'Unknown',
    occurredAt: partial.occurredAt ?? '2026-08-01T00:00:00.000Z',
    offsetMs: partial.offsetMs ?? 0,
    durationMs: partial.durationMs ?? 0,
    details: partial.details ?? {},
    payload: partial.payload ?? null,
    tags: partial.tags ?? [],
    relatedEventIds: partial.relatedEventIds ?? [],
  }
}

test('CAUSALITY 1/2: explicit causation creates an edge; chronology alone does not', () => {
  const events = [
    ev({ id: 'a', kind: 'Command', system: 'API Gateway', offsetMs: 0 }),
    ev({ id: 'b', kind: 'DomainEvent', system: 'Domain Model', causationId: 'a', offsetMs: 100 }),
    ev({ id: 'c', kind: 'Workflow', system: 'Workflow Engine', offsetMs: 200 }), // later but no causation
  ]
  const pairs = buildCausalEventPairs(events)
  assert.equal(pairs.length, 1, 'only explicit causation creates an edge')
  assert.equal(pairs[0].from.id, 'a')
  assert.equal(pairs[0].to.id, 'b')
})

test('CAUSALITY 3: unresolved causation remains unresolved', () => {
  const events = [ev({ id: 'a', causationId: 'missing-cmd', kind: 'Command' })]
  const unresolved = buildUnresolvedCauses(events)
  assert.equal(unresolved.length, 1)
  assert.equal(unresolved[0].causeId, 'missing-cmd')
  assert.equal(unresolved[0].event.id, 'a')
})

test('CAUSALITY 4: selection returns exact parents and children', () => {
  const events = [
    ev({ id: 'a' }),
    ev({ id: 'b', causationId: 'a' }),
    ev({ id: 'c', causationId: 'b' }),
  ]
  const mid = buildSelectionCausality(events, 'b')
  assert.deepEqual(mid.parents, ['a'])
  assert.deepEqual(mid.children, ['c'])
  const root = buildSelectionCausality(events, 'a')
  assert.deepEqual(root.parents, [])
  assert.deepEqual(root.children, ['b'])
})

test('SWIMLANE 6/10: events group by truthful system and appear once', () => {
  const events = [
    ev({ id: 'a', system: 'Workflow Engine' }),
    ev({ id: 'b', system: 'BoldSign' }),
    ev({ id: 'c', system: 'Workflow Engine' }),
  ]
  const lanes = groupEventsBySystem(events)
  assert.equal(lanes.length, 2)
  const wf = lanes.find((l) => l.system === 'Workflow Engine')!
  assert.equal(wf.events.length, 2)
  // total unique events preserved (each appears exactly once)
  assert.equal(lanes.reduce((n, l) => n + l.events.length, 0), 3)
})

test('SWIMLANE 7: unknown producer gets Unknown lane, never BoldSign/PostgreSQL', () => {
  const events = [
    ev({ id: 'a', system: 'Unknown', kind: 'Integration', title: 'docusign event' }),
    ev({ id: 'b', system: 'Unknown', kind: 'Persistence', title: 'storage event' }),
  ]
  const lanes = groupEventsBySystem(events)
  assert.equal(lanes.length, 1)
  assert.equal(lanes[0].system, 'Unknown')
  assert.equal(lanes[0].events.length, 2)
  // raw producers survive on each event (via the raw projection)
  const raw = rawEventFields(events[0])
  assert.ok(raw.some((f) => f.key === 'rawSystem' && f.value === 'Unknown'))
})

test('RAW 11/13/14: immutable ids visible, raw survives, metadata unchanged', () => {
  const e = ev({
    id: 'evt-1',
    correlationId: 'corr-x',
    causationId: 'cmd-9',
    workflowNodeId: 'pns_execution',
    system: 'Unknown',
    type: 'SIGNATURE_SENT',
    status: 'Unknown',
    payload: { qa_simulation: true, provider: 'docusign', nested: { a: 1 } },
  })
  const fields = rawEventFields(e)
  assert.ok(fields.some((f) => f.key === 'eventId' && f.value === 'evt-1'))
  assert.ok(fields.some((f) => f.key === 'causationId' && f.value === 'cmd-9'))
  assert.ok(fields.some((f) => f.key === 'workflowNodeId' && f.value === 'pns_execution'))
  assert.ok(fields.some((f) => f.key === 'qaSimulation' && f.value === 'true'))
  // The payload object itself is untouched by the raw projection.
  assert.deepEqual(e.payload, { qa_simulation: true, provider: 'docusign', nested: { a: 1 } })
  assert.equal((e.payload as { provider: string }).provider, 'docusign', 'raw provider preserved')
})

test('RAW 15: QA simulation marker remains visible', () => {
  const e = ev({ id: 'x', payload: { qa_simulation: true, provider: 'qa-simulation' } })
  const fields = rawEventFields(e)
  assert.ok(fields.some((f) => f.key === 'qaSimulation' && f.value === 'true'))
  assert.deepEqual(e.payload, { qa_simulation: true, provider: 'qa-simulation' })
})
