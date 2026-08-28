import test from 'node:test'
import assert from 'node:assert/strict'

import {
  classifyEvent,
  buildCausalGraph,
  layoutGraph,
} from '../../lib/causal-graph'
import type { TimelineEntry } from '../../lib/runtime-inspector'

// CAUSAL-GRAPH — pure, deterministic DAG projection + layered layout for the
// Runtime Inspector's causal overlay.

function entry(partial: Partial<TimelineEntry> & { id: string }): TimelineEntry {
  return {
    id: partial.id,
    occurredAt: partial.occurredAt ?? '2026-08-28T14:00:00.000Z',
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
  }
}

test('classifyEvent maps trace event types to subsystem colors', () => {
  assert.equal(classifyEvent('COMMAND_RECEIVED'), 'command')
  assert.equal(classifyEvent('DOMAIN_EVENT_EMITTED'), 'domain')
  assert.equal(classifyEvent('NODE_ENTERED'), 'workflow')
  assert.equal(classifyEvent('WORKFLOW_STARTED'), 'workflow')
  assert.equal(classifyEvent('TASK_COMPLETED'), 'task')
  assert.equal(classifyEvent('TIMER_FIRED'), 'task')
  assert.equal(classifyEvent('SIGNATURE_COMPLETED'), 'external')
  assert.equal(classifyEvent('DOCUMENT_CREATED'), 'persistence')
  assert.equal(classifyEvent('FAILURE'), 'failure')
  assert.equal(classifyEvent('WORKFLOW_FAILED'), 'failure')
  assert.equal(classifyEvent('SOMETHING_ELSE'), 'neutral')
})

test('buildCausalGraph links a domain event to its causing command via causationId', () => {
  const cmd = entry({
    id: 'c-recv', commandId: 'cmd-1', eventType: 'COMMAND_RECEIVED',
    system: 'command', summary: 'Command deal.create received',
  })
  const cmdDone = entry({
    id: 'c-done', commandId: 'cmd-1', eventType: 'COMMAND_COMPLETED',
    system: 'command', summary: 'Command deal.create success', outcome: 'success',
    occurredAt: '2026-08-28T14:00:01.000Z',
  })
  const evt = entry({
    id: 'e-1', domainEventId: 'evt-1', eventType: 'DOMAIN_EVENT_EMITTED',
    system: 'domain', causationId: 'cmd-1', commandId: 'cmd-1',
    summary: 'Domain event DealCreated',
  })
  const graph = buildCausalGraph([cmd, cmdDone, evt])
  assert.equal(graph.nodes.length, 2) // one command node (2 stages), one domain node
  const cmdNode = graph.nodes.find((n) => n.system === 'command')!
  const domNode = graph.nodes.find((n) => n.system === 'domain')!
  assert.equal(cmdNode.count, 2) // received + completed fused
  assert.equal(cmdNode.label, 'deal.create')
  assert.equal(domNode.label, 'DealCreated')
  assert.equal(graph.edges.length, 1)
  assert.equal(graph.edges[0].source, cmdNode.id) // cause (command) -> effect (domain)
  assert.equal(graph.edges[0].target, domNode.id)
})

test('buildCausalGraph drops dangling causation targets without error', () => {
  const evt = entry({
    id: 'e-1', domainEventId: 'evt-1', eventType: 'DOMAIN_EVENT_EMITTED',
    system: 'domain', causationId: 'no-such-command', summary: 'Domain event X',
  })
  const graph = buildCausalGraph([evt])
  assert.equal(graph.nodes.length, 1)
  assert.equal(graph.edges.length, 0)
})

test('collapseLinearChains fuses a linear same-subsystem persistence run', () => {
  const a = entry({ id: 'a', eventType: 'DOCUMENT_CREATED', system: 'doc', summary: 'write a' })
  const b = entry({ id: 'b', eventType: 'DOCUMENT_CREATED', system: 'doc', causationId: 'a', summary: 'write b' })
  const c = entry({ id: 'c', eventType: 'DOCUMENT_CREATED', system: 'doc', causationId: 'b', summary: 'write c' })
  const graph = buildCausalGraph([a, b, c])
  assert.equal(graph.nodes.length, 1)
  assert.equal(graph.nodes[0].count, 3)
  assert.equal(graph.edges.length, 0)
})

test('collapseLinearChains does NOT fuse a command with its domain event', () => {
  const cmd = entry({ id: 'c', commandId: 'cmd-1', eventType: 'COMMAND_COMPLETED', system: 'command' })
  const evt = entry({
    id: 'e', domainEventId: 'evt-1', eventType: 'DOMAIN_EVENT_EMITTED',
    system: 'domain', causationId: 'cmd-1', commandId: 'cmd-1',
  })
  const graph = buildCausalGraph([cmd, evt])
  assert.equal(graph.nodes.length, 2) // different colors -> never fused
  assert.equal(graph.edges.length, 1)
})

test('layoutGraph places a root at layer 0 and its child one layer right', () => {
  const cmd = entry({ id: 'c', commandId: 'cmd-1', eventType: 'COMMAND_COMPLETED', system: 'command' })
  const evt = entry({
    id: 'e', domainEventId: 'evt-1', eventType: 'DOMAIN_EVENT_EMITTED',
    system: 'domain', causationId: 'cmd-1', commandId: 'cmd-1',
  })
  const graph = buildCausalGraph([cmd, evt])
  const layout = layoutGraph(graph)
  assert.equal(layout.nodes.length, 2)
  const cmdL = layout.nodes.find((n) => n.system === 'command')!
  const domL = layout.nodes.find((n) => n.system === 'domain')!
  assert.equal(cmdL.layer, 0)
  assert.equal(domL.layer, 1)
  assert.ok(domL.x > cmdL.x)
})

test('layoutGraph survives a causation cycle without hanging', () => {
  // different subsystems (task vs persistence) so the pair never fuses; the
  // longest-path layering must still terminate via its cycle guard.
  const a = entry({ id: 'a', eventType: 'TIMER_FIRED', system: 'timer', causationId: 'b' })
  const b = entry({ id: 'b', eventType: 'DOCUMENT_CREATED', system: 'doc', causationId: 'a' })
  const graph = buildCausalGraph([a, b])
  const layout = layoutGraph(graph)
  assert.equal(layout.nodes.length, 2)
  assert.ok(layout.width > 0 && layout.height > 0)
})

test('layoutGraph returns an empty box for no events', () => {
  const layout = layoutGraph({ nodes: [], edges: [] })
  assert.equal(layout.nodes.length, 0)
  assert.ok(layout.width > 0 && layout.height > 0)
})

test('a domain node never claims the causing command id, regardless of order', () => {
  // Domain row processed BEFORE the command: its `commandId` is the CAUSING
  // command's id. The command->domain edge must still resolve, never short to
  // self. This is the exact live-data shape the dispatcher records.
  const evt = entry({
    id: 'e-1', domainEventId: 'evt-1', eventType: 'DOMAIN_EVENT_EMITTED',
    system: 'domain', commandId: 'cmd-1', causationId: 'cmd-1',
    summary: 'Domain event ClosingDateSet',
  })
  const cmd = entry({
    id: 'c-1', commandId: 'cmd-1', eventType: 'COMMAND_COMPLETED',
    system: 'command', summary: 'Command deal.set_closing_date success',
  })
  const graph = buildCausalGraph([evt, cmd]) // domain first
  const cmdNode = graph.nodes.find((n) => n.system === 'command')!
  const domNode = graph.nodes.find((n) => n.system === 'domain')!
  assert.equal(graph.nodes.length, 2)
  assert.equal(graph.edges.length, 1)
  assert.equal(graph.edges[0].source, cmdNode.id) // cause (command)
  assert.equal(graph.edges[0].target, domNode.id) // effect (domain)
})

test('collapseLinearChains retargets a branch edge out of a fused chain', () => {
  // chain a -> b, and b -> d where d is a different color, so b is a tail
  const a = entry({ id: 'a', eventType: 'DOCUMENT_CREATED', system: 'doc', summary: 'a' })
  const b = entry({ id: 'b', eventType: 'DOCUMENT_CREATED', system: 'doc', causationId: 'a', summary: 'b' })
  const d = entry({ id: 'd', eventType: 'SIGNATURE_SENT', system: 'sig', causationId: 'b', summary: 'd' })
  const graph = buildCausalGraph([a, b, d])
  const chain = graph.nodes.find((n) => n.count === 2)
  const sig = graph.nodes.find((n) => n.system === 'sig')
  assert.ok(chain)
  assert.ok(sig)
  assert.equal(graph.edges.length, 1)
  assert.equal(graph.edges[0].source, chain!.id) // branch retargeted to fused head
  assert.equal(graph.edges[0].target, sig!.id)
})

