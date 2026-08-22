import { test } from 'node:test'
import assert from 'node:assert/strict'
import { validateApplicationContract } from '../definitions/application-contract'
import { parseReSupermodel } from '../definitions/re-supermodel'
import { validateProcessGraph } from '../xml/graph-validator'
import type { ProcessGraph } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// ENG-14 — Layer 4: application contract validation.
//
// Every <command-node> references an application command (`commandType`) that
// the embedding application's port must be able to execute. An unknown command
// fails HERE, at deploy time — never during live execution when the router has
// no case for it. Pure module; no database, no packages.
// ---------------------------------------------------------------------------

function graph(nodes: ProcessGraph['nodes'], startNodeId = 'start'): ProcessGraph {
  return { startNodeId, nodes }
}

test('ENG-14: an unknown application command fails application-contract validation', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'cmd' }] },
    cmd: {
      id: 'cmd',
      type: 'command',
      commandType: 'deal.set_unknown_thing',
      transitions: [{ name: 'ok', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateApplicationContract(g)
  assert.equal(result.valid, false)
  assert.ok(
    result.errors.some((e) =>
      /command node references application command 'deal\.set_unknown_thing' which has no router case/.test(
        e,
      ),
    ),
    `expected an unrouted-command diagnostic, got: ${result.errors.join('; ')}`,
  )
  assert.ok(
    result.errors.some((e) => /register it in the canonical command inventory/.test(e)),
    'the diagnostic must be actionable',
  )
})

test('ENG-14: a known XML command-node type passes application-contract validation', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'cmd' }] },
    cmd: {
      id: 'cmd',
      type: 'command',
      commandType: 'deal.set_stage_under_contract',
      transitions: [{ name: 'ok', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateApplicationContract(g)
  assert.equal(result.valid, true, result.errors.join('; '))
  assert.deepEqual(result.errors, [])
})

test('ENG-14: application-only routed commands (never XML command-nodes) are still routable', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'cmd' }] },
    cmd: {
      id: 'cmd',
      type: 'command',
      commandType: 'deal.set_financing_type',
      transitions: [{ name: 'ok', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  })
  const result = validateApplicationContract(g)
  assert.equal(result.valid, true, result.errors.join('; '))
})

test('ENG-14: a definition with no command nodes passes application-contract validation', () => {
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  })
  const result = validateApplicationContract(g)
  assert.equal(result.valid, true, result.errors.join('; '))
})

test('ENG-14: RE_supermodel-v1 passes application-contract validation (all command-nodes routed)', () => {
  const parsed = parseReSupermodel()
  const result = validateApplicationContract(parsed.graph)
  assert.equal(result.valid, true, result.errors.join('; '))
})

test('ENG-14: a graph that fails layer 3 also fails overall (layers compose, not replace)', () => {
  // Unknown command + unreachable node: both layers must report their own error.
  const g = graph({
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'cmd' }] },
    cmd: {
      id: 'cmd',
      type: 'command',
      commandType: 'deal.set_unknown_thing',
      transitions: [{ name: 'ok', to: 'end' }],
    },
    orphan: { id: 'orphan', type: 'state' },
    end: { id: 'end', type: 'end' },
  })
  const graphResult = validateProcessGraph(g)
  assert.ok(
    graphResult.errors.some((e) => /'orphan' is unreachable/.test(e)),
    'layer 3 must report the unreachable node',
  )
  const result = validateApplicationContract(g)
  assert.equal(result.valid, false)
  assert.ok(result.errors.some((e) => /no router case/.test(e)))
})
