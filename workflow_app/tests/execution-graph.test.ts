import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  fakeEdgeCandidates,
  planSmithLayers,
  splitEligibility,
  type SmithWorkNode,
} from '../forge/execution-graph'

// ENG-FORGE-HARDEN-05 — Lead PRE dependency graph + fake-edge test. Parallelize
// only proven-independent work; real edges stay sequential; ambiguity is
// conservative.

function n(id: string, over: Partial<SmithWorkNode> = {}): SmithWorkNode {
  return { id, purpose: id, inputs: [], outputs: [], dependsOn: [], scope: id, ...over }
}

test('independent A/B -> SPLIT eligible (one parallel layer)', () => {
  const nodes = [
    n('A', { outputs: ['api'] }),
    n('B', { outputs: ['ui'] }),
  ]
  assert.deepEqual(splitEligibility(nodes), { eligible: true, reason: 'siblings are pairwise independent' })
  const plan = planSmithLayers({ nodes, concurrencyCap: 2 })
  assert.deepEqual(plan.layers, [['A', 'B']])
})

test('B depends on A -> sequential, NOT split', () => {
  const nodes = [
    n('A', { outputs: ['api'] }),
    n('B', { inputs: ['api'], dependsOn: ['A'] }),
  ]
  assert.equal(splitEligibility(nodes).eligible, false)
  const plan = planSmithLayers({ nodes, concurrencyCap: 2 })
  assert.deepEqual(plan.layers, [['A'], ['B']])
})

test('A/B independent, then C depends on both -> correct DAG (A,B parallel then C)', () => {
  const nodes = [
    n('A', { outputs: ['api'] }),
    n('B', { outputs: ['ui'] }),
    n('C', { inputs: ['api', 'ui'], dependsOn: ['A', 'B'] }),
  ]
  const plan = planSmithLayers({ nodes, concurrencyCap: 2 })
  assert.deepEqual(plan.layers, [['A', 'B'], ['C']])
})

test('ambiguous/unknown dependency -> conservative (invalid plan reported)', () => {
  const nodes = [n('A'), n('B', { dependsOn: ['GHOST'] })]
  const plan = planSmithLayers({ nodes, concurrencyCap: 2 })
  assert.equal(plan.valid, false)
  assert.ok(plan.errors.some((e) => e.includes('GHOST')))
})

test('a dependency cycle is rejected, never planned', () => {
  const nodes = [n('A', { dependsOn: ['B'] }), n('B', { dependsOn: ['A'] })]
  const plan = planSmithLayers({ nodes, concurrencyCap: 2 })
  assert.equal(plan.valid, false)
  assert.ok(plan.errors.some((e) => e.includes('cycle')))
})

test('concurrency cap is respected and never exceeded per layer', () => {
  const nodes = ['A', 'B', 'C', 'D', 'E'].map((id) => n(id))
  const plan = planSmithLayers({ nodes, concurrencyCap: 2 })
  const flat = plan.layers.flat()
  assert.equal(flat.length, 5)
  assert.ok(plan.layers.every((layer) => layer.length <= 2), 'no layer exceeds the cap')
  assert.deepEqual(new Set(flat), new Set(['A', 'B', 'C', 'D', 'E']))
})

test('fake-edge detector: an edge with no output/input overlap is fake', () => {
  const nodes = [
    n('A', { outputs: ['result_api'] }),
    n('B', { inputs: [], dependsOn: ['A'] }), // declares no dependency on A's output
  ]
  const fake = fakeEdgeCandidates(nodes)
  assert.deepEqual(fake, [{ from: 'A', to: 'B' }])
})

test('real edge is NOT flagged fake', () => {
  const nodes = [
    n('A', { outputs: ['result_api'] }),
    n('B', { inputs: ['result_api'], dependsOn: ['A'] }),
  ]
  assert.deepEqual(fakeEdgeCandidates(nodes), [])
})
