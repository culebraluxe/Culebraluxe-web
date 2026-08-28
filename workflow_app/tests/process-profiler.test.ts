import test from 'node:test'
import assert from 'node:assert/strict'

import { profileProcess } from '../../lib/process-profiler'
import type { TimelineEntry } from '../../lib/runtime-inspector'

// PROCESS-PROFILER — machine / human / external wait classification.

const T0 = '2026-08-28T14:00:00.000Z'
function at(offsetMs: number): string {
  return new Date(new Date(T0).getTime() + offsetMs).toISOString()
}

function entry(partial: Partial<TimelineEntry> & { id: string }): TimelineEntry {
  return {
    id: partial.id,
    occurredAt: partial.occurredAt ?? T0,
    relativeMs: partial.relativeMs ?? 0,
    eventType: partial.eventType ?? 'NODE_ENTERED',
    system: partial.system ?? 'workflow',
    summary: partial.summary ?? null,
    outcome: partial.outcome ?? null,
    durationMs: partial.durationMs ?? null,
    nodeId: partial.nodeId ?? null,
    causationId: partial.causationId ?? null,
    commandId: partial.commandId ?? null,
    domainEventId: partial.domainEventId ?? null,
    metadata: partial.metadata ?? null,
  }
}

test('a task-gated process attributes the idle window to a human wait', () => {
  const nodeTypes = { start: 'start', review: 'task', end: 'end' }
  const timeline = [
    entry({ id: '1', eventType: 'WORKFLOW_STARTED', nodeId: 'start', occurredAt: at(0) }),
    entry({ id: '2', eventType: 'NODE_ENTERED', nodeId: 'start', occurredAt: at(1) }),
    entry({ id: '3', eventType: 'TRANSITION_TAKEN', nodeId: 'start', occurredAt: at(2) }),
    entry({ id: '4', eventType: 'NODE_ENTERED', nodeId: 'review', occurredAt: at(3) }),
    // human completes after 60s; the leaving transition marks the end of the wait
    entry({ id: '5', eventType: 'TRANSITION_TAKEN', nodeId: 'review', occurredAt: at(60_003) }),
    entry({ id: '6', eventType: 'NODE_ENTERED', nodeId: 'end', occurredAt: at(60_004) }),
    entry({ id: '7', eventType: 'WORKFLOW_COMPLETED', occurredAt: at(60_010) }),
  ]
  const profile = profileProcess(nodeTypes, timeline)
  assert.equal(profile.hasWorkflowEvidence, true)
  assert.equal(profile.breakdown.human.count >= 1, true)
  assert.ok(profile.breakdown.human.durationMs >= 60_000, 'human wait is the dominant idle window')
  assert.equal(profile.nodeWaits.length, 1)
  assert.equal(profile.nodeWaits[0].nodeId, 'review')
  assert.equal(profile.nodeWaits[0].category, 'human')
  assert.ok(profile.nodeWaits[0].durationMs >= 60_000)
})

test('a timer node yields an external wait', () => {
  const nodeTypes = { start: 'start', delay: 'timer', end: 'end' }
  const timeline = [
    entry({ id: '1', eventType: 'NODE_ENTERED', nodeId: 'start', occurredAt: at(0) }),
    entry({ id: '2', eventType: 'TRANSITION_TAKEN', nodeId: 'start', occurredAt: at(1) }),
    entry({ id: '3', eventType: 'NODE_ENTERED', nodeId: 'delay', occurredAt: at(2) }),
    entry({ id: '4', eventType: 'TRANSITION_TAKEN', nodeId: 'delay', occurredAt: at(120_000 + 2) }),
    entry({ id: '5', eventType: 'NODE_ENTERED', nodeId: 'end', occurredAt: at(120_003) }),
    entry({ id: '6', eventType: 'WORKFLOW_COMPLETED', occurredAt: at(120_004) }),
  ]
  const profile = profileProcess(nodeTypes, timeline)
  assert.equal(profile.breakdown.external.durationMs >= 120_000, true)
  assert.equal(profile.nodeWaits[0].category, 'external')
  assert.equal(profile.nodeWaits[0].nodeId, 'delay')
})

test('a command-only trace (no workflow evidence) is all machine time', () => {
  const timeline = [
    entry({ id: '1', eventType: 'COMMAND_RECEIVED', system: 'command', commandId: 'cmd-1', occurredAt: at(0) }),
    entry({ id: '2', eventType: 'COMMAND_COMPLETED', system: 'command', commandId: 'cmd-1', occurredAt: at(50) }),
    entry({ id: '3', eventType: 'DOMAIN_EVENT_EMITTED', system: 'domain', domainEventId: 'evt-1', occurredAt: at(60) }),
  ]
  const profile = profileProcess({}, timeline)
  assert.equal(profile.hasWorkflowEvidence, false)
  assert.equal(profile.breakdown.machine.durationMs, 60)
  assert.equal(profile.breakdown.human.durationMs, 0)
  assert.equal(profile.breakdown.external.durationMs, 0)
})

test('empty timeline yields an empty profile', () => {
  const profile = profileProcess({ start: 'start' }, [])
  assert.equal(profile.totalMs, 0)
  assert.equal(profile.segments.length, 0)
  assert.equal(profile.hasWorkflowEvidence, false)
})

test('breakdown percentages are relative to total elapsed', () => {
  const nodeTypes = { start: 'start', review: 'task', end: 'end' }
  const timeline = [
    entry({ id: '1', eventType: 'NODE_ENTERED', nodeId: 'start', occurredAt: at(0) }),
    entry({ id: '2', eventType: 'TRANSITION_TAKEN', nodeId: 'start', occurredAt: at(10) }),
    entry({ id: '3', eventType: 'NODE_ENTERED', nodeId: 'review', occurredAt: at(20) }),
    entry({ id: '4', eventType: 'TRANSITION_TAKEN', nodeId: 'review', occurredAt: at(1020) }),
    entry({ id: '5', eventType: 'NODE_ENTERED', nodeId: 'end', occurredAt: at(1030) }),
    entry({ id: '6', eventType: 'WORKFLOW_COMPLETED', occurredAt: at(1040) }),
  ]
  const profile = profileProcess(nodeTypes, timeline)
  const pctSum = profile.breakdown.machine.pct + profile.breakdown.human.pct + profile.breakdown.external.pct
  assert.ok(Math.abs(pctSum - 100) < 1, `percentages sum to ~100, got ${pctSum}`)
  assert.equal(profile.totalMs, 1040)
})
