import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { NodeDefinition, ProcessGraph } from '../../workflow_engine/lib/workflow/types'
import {
  diffProcessGraphs,
  compatibilityDiagnostics,
  isRollbackDeployment,
  graphsEqual,
} from '../definitions/compatibility'

// ---------------------------------------------------------------------------
// ENG-12 — Workflow Definition Versioning / Compatibility diagnostics.
//
// Pure module tests (no database): the diff between two deployed graphs of the
// same logical key, the running-instance impact statements, rollback-as-new-
// version detection, duplicate detection, and the explicit cross-version
// migration statement.
// ---------------------------------------------------------------------------

function task(id: string, name: string, to: string | null): NodeDefinition {
  return {
    id,
    type: 'task',
    name,
    ...(to ? { transitions: [{ name: 'done', to }] } : {}),
  }
}

/** v1 graph: start -> review -> approve -> end. */
function v1Graph(): ProcessGraph {
  return {
    startNodeId: 'start',
    nodes: {
      start: {
        id: 'start',
        type: 'start',
        transitions: [{ name: 'begin', to: 'review' }],
      },
      review: task('review', 'Review', 'approve'),
      approve: task('approve', 'Approve', 'end'),
      end: { id: 'end', type: 'end' },
    },
    displayOrder: ['start', 'review', 'approve', 'end'],
  }
}

/** v2 graph: review renamed, legacy node added after approve. */
function v2Graph(): ProcessGraph {
  return {
    startNodeId: 'start',
    nodes: {
      start: {
        id: 'start',
        type: 'start',
        transitions: [{ name: 'begin', to: 'review' }],
      },
      review: task('review', 'Review (v2)', 'approve'),
      approve: task('approve', 'Approve', 'legacy'),
      legacy: task('legacy', 'Legacy Follow-up', 'end'),
      end: { id: 'end', type: 'end' },
    },
    displayOrder: ['start', 'review', 'approve', 'legacy', 'end'],
  }
}

// ---------------------------------------------------------------------------
// diffProcessGraphs
// ---------------------------------------------------------------------------

test('identical graphs produce an empty diff', () => {
  const diff = diffProcessGraphs(v1Graph(), v1Graph())
  assert.equal(diff.identical, true)
  assert.deepEqual(diff.addedNodeIds, [])
  assert.deepEqual(diff.removedNodeIds, [])
  assert.deepEqual(diff.renamedNodes, [])
  assert.deepEqual(diff.changedNodes, [])
  assert.equal(diff.changedStartNode, false)
  assert.equal(diff.displayOrderChanged, false)
})

test('added nodes are reported; nothing else changes', () => {
  const diff = diffProcessGraphs(v1Graph(), v2Graph())
  assert.equal(diff.identical, false)
  assert.deepEqual(diff.addedNodeIds, ['legacy'])
  assert.deepEqual(diff.removedNodeIds, [])
  assert.deepEqual(diff.renamedNodes, [{ nodeId: 'review', from: 'Review', to: 'Review (v2)' }])
})

test('removed nodes are reported', () => {
  const removed = { ...v1Graph() }
  const next = structuredClone(v1Graph())
  delete (next.nodes as Record<string, NodeDefinition>).approve
  next.nodes.end = { id: 'end', type: 'end' }
  ;(next.nodes.review as NodeDefinition).transitions = [{ name: 'done', to: 'end' }]

  const diff = diffProcessGraphs(removed, next)
  assert.equal(diff.identical, false)
  assert.deepEqual(diff.removedNodeIds, ['approve'])
  assert.deepEqual(diff.addedNodeIds, [])
})

test('renamed nodes (same id, different name) are reported as renames, not removals', () => {
  const diff = diffProcessGraphs(v1Graph(), v2Graph())
  assert.deepEqual(diff.renamedNodes, [{ nodeId: 'review', from: 'Review', to: 'Review (v2)' }])
  // The state identity is the node id — the renamed node is NOT removed/added.
  assert.deepEqual(diff.removedNodeIds, [])
  assert.deepEqual(diff.addedNodeIds, ['legacy'])
})

test('structural changes (transitions/type/command/outcome) are reported per node', () => {
  const prev = v1Graph()
  const next = structuredClone(v1Graph())
  const review = next.nodes.review as NodeDefinition
  review.transitions = [{ name: 'escalate', to: 'approve' }] // renamed transition
  ;(next.nodes.approve as NodeDefinition).type = 'state' // type change
  next.nodes.end = { id: 'end', type: 'end', outcome: 'failed' } // outcome change

  const diff = diffProcessGraphs(prev, next)
  assert.equal(diff.identical, false)
  const byId = new Map(diff.changedNodes.map((c) => [c.nodeId, c.changes]))
  assert.ok(byId.get('review')!.some((c) => c.includes('transitions changed')))
  assert.ok(byId.get('approve')!.some((c) => c.includes('type task -> state')))
  assert.ok(byId.get('end')!.some((c) => c.includes('outcome completed -> failed')))
})

test('a changed start node is reported', () => {
  const next = structuredClone(v1Graph())
  next.startNodeId = 'other_start'
  const diff = diffProcessGraphs(v1Graph(), next)
  assert.equal(diff.changedStartNode, true)
  assert.equal(diff.identical, false)
})

test('a display-order-only change does not make the executable graph different', () => {
  const next = structuredClone(v1Graph())
  next.displayOrder = ['end', 'start', 'review', 'approve'] // presentation only
  const diff = diffProcessGraphs(v1Graph(), next)
  assert.equal(diff.identical, true, 'executable graph is unchanged')
  assert.equal(diff.displayOrderChanged, true)
})

// ---------------------------------------------------------------------------
// compatibilityDiagnostics
// ---------------------------------------------------------------------------

test('identical graphs yield the rollback-as-new-version diagnostic', () => {
  const report = compatibilityDiagnostics(v1Graph(), v1Graph(), {
    key: 'quote_flow',
    previousVersion: 1,
    nextVersion: 3,
  })
  assert.equal(report.rollback.isRollback, true)
  assert.match(report.rollback.detail, /rollback-as-new-version/)
  assert.match(report.rollback.detail, /NEW version number/)
  assert.match(report.rollback.detail, /v3/)
  assert.match(report.summary.join('\n'), /no executable-graph changes/)
})

test('a changed graph is explicitly NOT a rollback', () => {
  const report = compatibilityDiagnostics(v1Graph(), v2Graph(), {
    key: 'quote_flow',
    previousVersion: 1,
    nextVersion: 2,
  })
  assert.equal(report.rollback.isRollback, false)
  assert.match(report.rollback.detail, /not a rollback/)
  assert.match(report.rollback.detail, /1 added, 0 removed, 1 renamed/)
})

test('running-instance impact statements make the V1 policy explicit', () => {
  const report = compatibilityDiagnostics(v1Graph(), v2Graph())
  const impact = report.runningInstanceImpact.join('\n')
  assert.match(impact, /pinned to their exact definition_id forever/)
  assert.match(impact, /removed or renamed nodes in a new version affect only NEW instances/)
  assert.match(impact, /all deployed versions remain available/)
  assert.match(impact, /rollback means deploying a prior graph as a NEW version number/)
})

test('the cross-version migration statement is explicit and absolute', () => {
  const report = compatibilityDiagnostics(v1Graph(), v2Graph())
  assert.match(report.migrationUnsupported, /UNSUPPORTED/)
  assert.match(report.migrationUnsupported, /never switches to a newer definition version/)
  assert.match(report.migrationUnsupported, /completing or cancelling/)
})

test('summary lists added, removed, renamed and changed nodes deterministically', () => {
  const report = compatibilityDiagnostics(v1Graph(), v2Graph())
  const summary = report.summary.join('\n')
  assert.match(summary, /added node\(s\): legacy/)
  assert.match(summary, /renamed node\(s\): review \('Review' -> 'Review \(v2\)'\)/)
})

// ---------------------------------------------------------------------------
// rollback detection + duplicate detection
// ---------------------------------------------------------------------------

test('isRollbackDeployment is true only for the exact prior executable graph', () => {
  assert.equal(isRollbackDeployment(v1Graph(), structuredClone(v1Graph())), true)
  assert.equal(isRollbackDeployment(v1Graph(), v2Graph()), false)
})

test('isRollbackDeployment ignores display-order-only differences', () => {
  const next = structuredClone(v1Graph())
  next.displayOrder = ['approve', 'start']
  assert.equal(isRollbackDeployment(v1Graph(), next), true)
})

test('graphsEqual is key-order insensitive and includes displayOrder', () => {
  const a = v1Graph()
  const b = structuredClone(v1Graph())
  // Rebuild with a different insertion order — semantically identical.
  const reordered: Record<string, NodeDefinition> = {}
  for (const id of ['end', 'approve', 'review', 'start']) reordered[id] = (b.nodes as any)[id]
  b.nodes = reordered
  assert.equal(graphsEqual(a, b), true)

  const display = structuredClone(v1Graph())
  display.displayOrder = ['start', 'approve']
  assert.equal(graphsEqual(a, display), false, 'displayOrder is part of the stored definition')

  const changed = structuredClone(v1Graph())
  ;(changed.nodes.review as NodeDefinition).name = 'Review (v2)'
  assert.equal(graphsEqual(a, changed), false)
})
