import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { ProcessGraph } from '../../workflow_engine/lib/workflow/types'
import {
  definitionVersionPolicy,
  classifyDeploy,
  deploymentCompatibility,
  IMMUTABLE_DEFINITION_ERROR,
} from '../definitions/version-policy'

function graph(nodes: Record<string, any>): ProcessGraph {
  return { startNodeId: 'start', nodes: nodes as ProcessGraph['nodes'] }
}

const V1_GRAPH = graph({
  start: { id: 'start', type: 'start', transitions: [{ name: 'begin', to: 'review' }] },
  review: { id: 'review', type: 'task', name: 'Review', transitions: [{ name: 'ok', to: 'end' }] },
  end: { id: 'end', type: 'end' },
})

const V2_GRAPH = graph({
  start: { id: 'start', type: 'start', transitions: [{ name: 'begin', to: 'review' }] },
  review: { id: 'review', type: 'task', name: 'Review (v2)', transitions: [{ name: 'ok', to: 'end' }] },
  end: { id: 'end', type: 'end' },
})

test('a missing (key, version) row is new', () => {
  assert.deepEqual(definitionVersionPolicy(false, 0), { kind: 'new' })
})

test('an existing version with no instances is replaceable (draft iteration)', () => {
  assert.deepEqual(definitionVersionPolicy(true, 0), { kind: 'replaceable' })
})

test('an existing version with instances is immutable', () => {
  assert.deepEqual(definitionVersionPolicy(true, 1), { kind: 'immutable' })
  assert.deepEqual(definitionVersionPolicy(true, 99), { kind: 'immutable' })
})

test('the immutable error message names the refusal', () => {
  assert.match(IMMUTABLE_DEFINITION_ERROR, /Refusing to replace/)
})

// ---------------------------------------------------------------------------
// ENG-12 — classifyDeploy: the explicit deploy decision table.
// ---------------------------------------------------------------------------

test('deploy classification: no row inserts a new version', () => {
  const d = classifyDeploy(false, 0, null, V1_GRAPH)
  assert.deepEqual(d, { action: 'insert', created: true, reason: 'new' })
})

test('deploy classification: a byte-identical redeploy of an instance-less version is a duplicate update', () => {
  const d = classifyDeploy(true, 0, V1_GRAPH, structuredClone(V1_GRAPH))
  assert.deepEqual(d, {
    action: 'update',
    created: false,
    duplicate: true,
    reason: 'replaceable',
  })
})

test('deploy classification: a changed graph on an instance-less version is a draft iteration', () => {
  const d = classifyDeploy(true, 0, V1_GRAPH, V2_GRAPH)
  assert.deepEqual(d, {
    action: 'update',
    created: false,
    duplicate: false,
    reason: 'replaceable',
  })
})

test('deploy classification: a version with instances rejects in-place mutation', () => {
  const d = classifyDeploy(true, 1, V1_GRAPH, V2_GRAPH)
  assert.equal(d.action, 'reject')
  assert.equal(d.reason, 'immutable')
  assert.match((d as any).message, /Refusing to replace/)
})

test('deploy classification: a version with instances rejects even a byte-identical redeploy', () => {
  // Immutability is about the version having executed — identical content does
  // not make a write legal. This is the explicit rejection of in-place
  // mutation in its strictest form.
  const d = classifyDeploy(true, 1, V1_GRAPH, structuredClone(V1_GRAPH))
  assert.equal(d.action, 'reject')
  assert.equal(d.reason, 'immutable')
})

test('deploymentCompatibility couples the decision with diagnostics for the operator', () => {
  const report = deploymentCompatibility(true, 1, V1_GRAPH, V2_GRAPH, {
    key: 'quote_flow',
    previousVersion: 1,
    nextVersion: 2,
  })
  assert.equal(report.decision.action, 'reject')
  assert.ok(report.diagnostics, 'diagnostics are present when a previous graph exists')
  assert.equal(report.diagnostics!.rollback.isRollback, false)
  assert.deepEqual(report.diagnostics!.diff.addedNodeIds, [])

  const fresh = deploymentCompatibility(false, 0, null, V1_GRAPH)
  assert.equal(fresh.decision.action, 'insert')
  assert.equal(fresh.diagnostics, null, 'no previous graph -> no diff diagnostics')
})
