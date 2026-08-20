import { test } from 'node:test'
import assert from 'node:assert/strict'
import { validateProcessGraph } from '../xml/graph-validator'
import { parseReSupermodel } from '../definitions/re-supermodel'
import type { ProcessGraph } from '../../workflow_engine/lib/workflow/types'

function graph(nodes: ProcessGraph['nodes'], startNodeId = 'start'): ProcessGraph {
  return { startNodeId, nodes }
}

test('a well-formed graph is valid', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'work' }] },
    work: {
      id: 'work',
      type: 'task',
      name: 'Work',
      responsibility: 'brokerage',
      transitions: [{ name: 'done', to: 'end' }],
    },
    end: { id: 'end', type: 'end', outcome: 'completed' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, true)
  assert.equal(result.errors.length, 0)
})

test('missing start node is rejected', () => {
  const g = graph({ work: { id: 'work', type: 'task' } })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, false)
  assert.ok(result.errors.some((e) => /no node of type "start"/.test(e)))
})

test('startNodeId referencing a non-start node is rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'task', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, false)
  assert.ok(result.errors.some((e) => /type "start"/.test(e)))
})

test('multiple start nodes are rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'end' }] },
    start2: { id: 'start2', type: 'start', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, false)
  assert.ok(result.errors.some((e) => /2 start nodes/.test(e)))
})

test('missing transition target is rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'nope' }] },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, false)
  assert.ok(result.errors.some((e) => /targets missing node 'nope'/.test(e)))
})

test('duplicate transition names on a node are rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'start' },
    work: {
      id: 'work',
      type: 'task',
      transitions: [
        { name: 'go', to: 'end' },
        { name: 'go', to: 'end' },
      ],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.ok(result.errors.some((e) => /duplicate transition name 'go'/.test(e)))
})

test('command node without a commandType is rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'cmd' }] },
    cmd: { id: 'cmd', type: 'command', transitions: [{ name: 'ok', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, false)
  assert.ok(result.errors.some((e) => /command node 'cmd' has no commandType/.test(e)))
})

test('decision rule referencing a missing transition is rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'decide' }] },
    decide: {
      id: 'decide',
      type: 'decision',
      decisions: [{ condition: 'flag == true', transition: 'missing' }],
      transitions: [{ name: 'no', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.ok(result.errors.some((e) => /references transition 'missing'/.test(e)))
})

test('decision node with no transitions is rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'decide' }] },
    decide: { id: 'decide', type: 'decision', decisions: [] },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.ok(result.errors.some((e) => /decision node 'decide' has no transitions/.test(e)))
})

test('timer node with neither due-at nor due-at-variable is rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 't' }] },
    t: { id: 't', type: 'timer', timer: {}, transitions: [{ name: 'fire', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.ok(result.errors.some((e) => /timer node 't' must declare due-at or due-at-variable/.test(e)))
})

test('end node with an invalid outcome is rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end', outcome: 'banana' as any },
  })
  const result = validateProcessGraph(g)
  assert.ok(result.errors.some((e) => /invalid outcome 'banana'/.test(e)))
})

test('cycles are allowed (blocker loops are intentional)', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'work' }] },
    work: {
      id: 'work',
      type: 'task',
      transitions: [
        { name: 'done', to: 'end' },
        { name: 'issue', to: 'blocker' },
      ],
    },
    blocker: {
      id: 'blocker',
      type: 'task',
      transitions: [{ name: 'resolved', to: 'work' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, true, `expected cycles to be allowed: ${result.errors.join('; ')}`)
})

test('RE_supermodel graph validates cleanly', () => {
  const parsed = parseReSupermodel()
  const result = validateProcessGraph(parsed.graph)
  assert.equal(result.valid, true, result.errors.join('; '))
})

test('decision node with an unsupported condition expression is rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'decide' }] },
    decide: {
      id: 'decide',
      type: 'decision',
      decisions: [{ condition: 'flag && other', transition: 'no' }],
      transitions: [{ name: 'no', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.ok(result.errors.some((e) => /unsupported condition expression/.test(e)))
})
