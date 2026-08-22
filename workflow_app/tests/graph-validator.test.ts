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

// ---------------------------------------------------------------------------
// ENG-14 — Workflow Definition Validation / Static Analysis.
// ---------------------------------------------------------------------------

test('ENG-14: unreachable nodes are reported', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'work' }] },
    work: {
      id: 'work',
      type: 'task',
      name: 'Work',
      transitions: [{ name: 'done', to: 'end' }],
    },
    orphan: {
      id: 'orphan',
      type: 'task',
      name: 'Never executed',
      transitions: [{ name: 'done', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, false)
  assert.ok(
    result.errors.some((e) => /node 'orphan' is unreachable/.test(e)),
    `expected an unreachable diagnostic, got: ${result.errors.join('; ')}`,
  )
})

test('ENG-14: a node only reachable via a cycle back to itself is still reachable', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'work' }] },
    work: {
      id: 'work',
      type: 'task',
      transitions: [
        { name: 'again', to: 'blocker' },
        { name: 'done', to: 'end' },
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
  assert.equal(result.valid, true, result.errors.join('; '))
})

test('ENG-14: unsupported node types are rejected (no silent passthrough)', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'magic' }] },
    magic: { id: 'magic', type: 'teleport', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, false)
  assert.ok(
    result.errors.some((e) => /unsupported type 'teleport'/.test(e)),
    `expected an unsupported-type diagnostic, got: ${result.errors.join('; ')}`,
  )
  assert.ok(
    result.errors.some((e) => /silently treat it as a passthrough/.test(e)),
    'the diagnostic must explain the silent passthrough risk',
  )
})

test('ENG-14: subprocess is declared but unimplemented — rejected with a targeted diagnostic', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'sub' }] },
    sub: {
      id: 'sub',
      type: 'subprocess',
      subprocessKey: 'some_flow',
      transitions: [{ name: 'go', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, false)
  assert.ok(
    result.errors.some((e) => /'subprocess'.*no runtime implementation/.test(e)),
    `expected a subprocess-specific diagnostic, got: ${result.errors.join('; ')}`,
  )
})

test('ENG-14: a required fork branch that is a closed loop makes the join impossible — rejected', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'fork' }] },
    fork: {
      id: 'fork',
      type: 'fork',
      transitions: [
        { name: 'loop', to: 'loop_node' },
        { name: 'ok', to: 'join' },
      ],
    },
    // loop_node -> loop_node is a pure cycle with no exit: the token can never
    // complete, so the join can never release and the process would hang.
    loop_node: {
      id: 'loop_node',
      type: 'task',
      transitions: [{ name: 'again', to: 'loop_node' }],
    },
    join: {
      id: 'join',
      type: 'join',
      transitions: [{ name: 'go', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, false)
  assert.ok(
    result.errors.some((e) => /required branch 'loop' of fork 'fork'.*closed loop with no exit/.test(e)),
    `expected a closed-loop diagnostic, got: ${result.errors.join('; ')}`,
  )
  assert.ok(
    result.errors.some((e) => /can never release/.test(e)),
    'the diagnostic must state the join can never release',
  )
})

test('ENG-14: an intentional blocker loop inside a fork branch with an exit stays valid', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'fork' }] },
    fork: {
      id: 'fork',
      type: 'fork',
      transitions: [
        { name: 'work', to: 'work' },
        { name: 'ok', to: 'join' },
      ],
    },
    work: {
      id: 'work',
      type: 'task',
      transitions: [
        { name: 'issue', to: 'blocker' },
        { name: 'done', to: 'join' },
      ],
    },
    blocker: {
      id: 'blocker',
      type: 'task',
      transitions: [{ name: 'resolved', to: 'work' }],
    },
    join: {
      id: 'join',
      type: 'join',
      transitions: [{ name: 'go', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(
    result.valid,
    true,
    `intentional blocker loops with an exit must stay valid: ${result.errors.join('; ')}`,
  )
})

test('ENG-14: a required fork branch that never reaches a join is a warning, not an error', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'fork' }] },
    fork: {
      id: 'fork',
      type: 'fork',
      transitions: [
        { name: 'escape', to: 'escaped' },
        { name: 'ok', to: 'join' },
      ],
    },
    // 'escape' reaches only an end node — the join-wait for it is trivially
    // satisfied by termination. Warn, but the graph is still deployable.
    escaped: {
      id: 'escaped',
      type: 'task',
      transitions: [{ name: 'done', to: 'end' }],
    },
    join: {
      id: 'join',
      type: 'join',
      transitions: [{ name: 'go', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, true)
  assert.ok(
    result.warnings.some((w) => /required branch 'escape' of fork 'fork' never reaches a join/.test(w)),
    `expected a bypass warning, got: ${result.warnings.join('; ')}`,
  )
})

test('ENG-14: a required fork branch entering a nested fork before any join is a warning, not an error', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'fork' }] },
    fork: {
      id: 'fork',
      type: 'fork',
      transitions: [
        { name: 'nested', to: 'fork2' },
        { name: 'ok', to: 'join' },
      ],
    },
    fork2: {
      id: 'fork2',
      type: 'fork',
      transitions: [{ name: 'a', to: 'end' }],
    },
    join: {
      id: 'join',
      type: 'join',
      transitions: [{ name: 'go', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateProcessGraph(g)
  assert.equal(result.valid, true)
  assert.ok(
    result.warnings.some((w) => /required branch 'nested' of fork 'fork' passes through nested fork/.test(w)),
    `expected a nested-fork warning, got: ${result.warnings.join('; ')}`,
  )
})
