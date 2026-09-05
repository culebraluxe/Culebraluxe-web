import assert from 'node:assert/strict'
import test from 'node:test'

import {
  ensureForgeSdlcTopology,
  forgeSdlcTopology,
  resetForgeSdlcTopologyCache,
} from './forge-topology'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 — FORGE_SDLC superset as the live topology contract.
//
// Proves the authoritative superset loads (four-layer, FORGE inventory),
// declares a <dynamic-fork> SPLIT path, exposes the four termini, and satisfies
// the fail-closed structural invariants used by the wake guard.
//
// No database, no packages.
// ---------------------------------------------------------------------------

test('ENG-FORGE-V9: FORGE_SDLC superset loads as the live topology', () => {
  resetForgeSdlcTopologyCache()
  const topology = forgeSdlcTopology()
  assert.equal(topology.key, 'FORGE_SDLC')
  assert.equal(topology.version, 1)
  assert.equal(topology.dynamicForkId, 'split_dispatch')
  assert.ok(topology.nodeIds.has('classify_work'))
  assert.ok(topology.nodeIds.has('execution_shape'))
  assert.ok(topology.nodeIds.has('hold'))
})

test('ENG-FORGE-V9: the four termini are declared', () => {
  const topology = forgeSdlcTopology()
  for (const end of ['complete', 'cancelled', 'failed', 'archive_research']) {
    assert.ok(topology.endIds.has(end), `terminus '${end}' missing`)
  }
})

test('ENG-FORGE-V9: every task/command node carries a Forge position responsibility', () => {
  const topology = forgeSdlcTopology()
  const valid = new Set(['scout', 'architect', 'lead', 'smith', 'qa', 'dev_ops'])
  const tasks = Object.values(topology.tasks)
  assert.ok(tasks.length > 0, 'FORGE_SDLC must define task/command nodes')
  for (const task of tasks) {
    assert.ok(valid.has(task.responsibility!), `node '${task.id}' role '${task.responsibility}' must be a Forge position`)
  }
})

test('ENG-FORGE-V9: the split_dispatch fork is NOT a task node (dynamic fork present)', () => {
  const topology = forgeSdlcTopology()
  assert.equal(topology.dynamicForkId, 'split_dispatch')
  assert.equal(topology.tasks.split_dispatch, undefined)
})

test('ENG-FORGE-V9: ensureForgeSdlcTopology returns a cached valid topology (live guard)', () => {
  resetForgeSdlcTopologyCache()
  const a = ensureForgeSdlcTopology()
  const b = ensureForgeSdlcTopology()
  assert.equal(a, b, 'guard result must be cached per process')
  assert.equal(a.dynamicForkId, 'split_dispatch')
})
