import { test } from 'node:test'
import assert from 'node:assert/strict'
import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine'
import { FakeSql } from '../../workflow_engine/tests/fake-sql'
import { stubEvaluator } from '../../workflow_engine/tests/fixtures'
import type { NodeDefinition, ProcessGraph } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// ENG-12 — Workflow Definition Versioning / Compatibility (engine-level).
//
// Proves the V1 versioning policy against the real WorkflowEngine + in-memory
// FakeSql (no database):
//
//   - new instances use the newest deployed version
//   - an instance started on an explicit version stays pinned to that exact
//     definition_id forever
//   - removed nodes affect only NEW definitions — a running instance completes
//     through a node that a newer version deleted
//   - renamed nodes affect only NEW definitions — a running instance keeps its
//     original task names
//   - rollback = deploying a prior graph as a NEW version number; every
//     deployed version remains available
//
// The graphs are small, hand-authored ProcessGraphs (not the RE_supermodel),
// so the tests isolate versioning behavior from the closing model.
// ---------------------------------------------------------------------------

const KEY = 'quote_flow'

function node(type: string, id: string, opts: Partial<NodeDefinition> = {}): NodeDefinition {
  return { id, type, ...opts }
}

/** v1: start -> review -> legacy_check -> approve -> end. */
function v1Graph(): ProcessGraph {
  return {
    startNodeId: 'start',
    nodes: {
      start: node('start', 'start', { transitions: [{ name: 'begin', to: 'review' }] }),
      review: node('task', 'review', {
        name: 'Review',
        transitions: [{ name: 'ok', to: 'legacy_check' }],
      }),
      legacy_check: node('task', 'legacy_check', {
        name: 'Legacy Check',
        transitions: [{ name: 'ok', to: 'approve' }],
      }),
      approve: node('task', 'approve', {
        name: 'Approve',
        transitions: [{ name: 'done', to: 'end' }],
      }),
      end: node('end', 'end'),
    },
  }
}

/** v2: review renamed, legacy_check REMOVED, bonus node ADDED after approve. */
function v2Graph(): ProcessGraph {
  return {
    startNodeId: 'start',
    nodes: {
      start: node('start', 'start', { transitions: [{ name: 'begin', to: 'review' }] }),
      review: node('task', 'review', {
        name: 'Review (v2)',
        transitions: [{ name: 'ok', to: 'approve' }],
      }),
      approve: node('task', 'approve', {
        name: 'Approve',
        transitions: [{ name: 'done', to: 'bonus' }],
      }),
      bonus: node('task', 'bonus', {
        name: 'Bonus Review',
        transitions: [{ name: 'done', to: 'end' }],
      }),
      end: node('end', 'end'),
    },
  }
}

type AnyRow = Record<string, any>

function hasTask(fake: FakeSql, name: string, instanceId: string): boolean {
  return fake.store.tasks.some(
    (t) => t.process_instance_id === instanceId && t.name === name,
  )
}

function taskByName(fake: FakeSql, name: string, instanceId: string): AnyRow {
  const t = fake.store.tasks.find(
    (row) => row.process_instance_id === instanceId && row.name === name,
  )
  assert.ok(t, `expected task "${name}" on instance ${instanceId}`)
  return t!
}

function setup(v3 = false) {
  const fake = new FakeSql()
  const id1 = fake.seedDefinition(KEY, 1, v1Graph())
  const id2 = fake.seedDefinition(KEY, 2, v2Graph())
  const id3 = v3 ? fake.seedDefinition(KEY, 3, v1Graph()) : null // rollback: v1 graph as v3
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator })
  return { fake, engine, id1, id2, id3 }
}

// ---------------------------------------------------------------------------
// Instance pinning + newest-version resolution
// ---------------------------------------------------------------------------

test('new instances use the newest deployed version; explicit version selects the pinned one', async () => {
  const { fake, engine, id1, id2 } = setup()

  const latest = await engine.startProcess({ definitionKey: KEY, startedBy: 'broker' })
  const explicit = await engine.startProcess({ definitionKey: KEY, version: 1, startedBy: 'broker' })

  const latestPi = await engine.getProcessInstance(latest.processInstanceId)
  const explicitPi = await engine.getProcessInstance(explicit.processInstanceId)
  assert.equal(latestPi!.definitionId, id2, 'no version -> newest deployed version')
  assert.equal(explicitPi!.definitionId, id1, 'explicit version -> that exact version')

  // The newest instance runs the v2 graph (renamed review task)...
  assert.ok(hasTask(fake, 'Review (v2)', latest.processInstanceId))
  // ...and the explicit v1 instance runs the v1 graph.
  assert.ok(hasTask(fake, 'Review', explicit.processInstanceId))
})

test('a running instance stays pinned to its definition_id across newer deployments', async () => {
  const { fake, engine, id1 } = setup()

  const a = await engine.startProcess({ definitionKey: KEY, version: 1, startedBy: 'broker' })
  // Simulate "a newer deployment arrives" — v2 is already seeded; start another
  // instance that uses it.
  await engine.startProcess({ definitionKey: KEY, startedBy: 'broker' })

  // Complete work on A; every engine step re-loads the definition by
  // definition_id, so A continues on the v1 graph.
  await engine.completeTask({
    taskId: taskByName(fake, 'Review', a.processInstanceId).id,
    userId: 'sme',
    transitionName: 'ok',
  })
  const pi = await engine.getProcessInstance(a.processInstanceId)
  assert.equal(pi!.definitionId, id1, 'pinned to the exact v1 definition row')
  assert.ok(hasTask(fake, 'Legacy Check', a.processInstanceId), 'still executing the v1 graph')
})

// ---------------------------------------------------------------------------
// Removed / renamed nodes are safe for running instances
// ---------------------------------------------------------------------------

test('removed nodes affect only new definitions — a v1 instance completes through the removed node', async () => {
  const { fake, engine, id1 } = setup()

  const a = await engine.startProcess({ definitionKey: KEY, version: 1, startedBy: 'broker' })
  const b = await engine.startProcess({ definitionKey: KEY, startedBy: 'broker' }) // v2

  // A (v1) moves review -> legacy_check: the node exists in ITS graph.
  await engine.completeTask({
    taskId: taskByName(fake, 'Review', a.processInstanceId).id,
    userId: 'sme',
    transitionName: 'ok',
  })
  assert.ok(hasTask(fake, 'Legacy Check', a.processInstanceId))

  // B (v2) moves review -> approve directly: legacy_check was REMOVED in v2
  // and must never surface for the new instance.
  await engine.completeTask({
    taskId: taskByName(fake, 'Review (v2)', b.processInstanceId).id,
    userId: 'sme',
    transitionName: 'ok',
  })
  assert.equal(hasTask(fake, 'Legacy Check', b.processInstanceId), false)
  assert.ok(hasTask(fake, 'Approve', b.processInstanceId))

  // A completes THROUGH the node that v2 removed, all the way to the end.
  await engine.completeTask({
    taskId: taskByName(fake, 'Legacy Check', a.processInstanceId).id,
    userId: 'sme',
    transitionName: 'ok',
  })
  assert.ok(hasTask(fake, 'Approve', a.processInstanceId))
  await engine.completeTask({
    taskId: taskByName(fake, 'Approve', a.processInstanceId).id,
    userId: 'sme',
    transitionName: 'done',
  })
  const piA = await engine.getProcessInstance(a.processInstanceId)
  assert.equal(piA!.outcome, 'completed')
  assert.equal(piA!.definitionId, id1)
})

test('renamed nodes affect only new definitions — a v1 instance keeps its original task name', async () => {
  const { fake, engine } = setup()

  const a = await engine.startProcess({ definitionKey: KEY, version: 1, startedBy: 'broker' })
  const b = await engine.startProcess({ definitionKey: KEY, startedBy: 'broker' }) // v2

  assert.ok(hasTask(fake, 'Review', a.processInstanceId), 'v1 instance: v1 task name')
  assert.ok(hasTask(fake, 'Review (v2)', b.processInstanceId), 'v2 instance: renamed task name')
  assert.equal(hasTask(fake, 'Review', b.processInstanceId), false)

  // Completing the v1-named task on A works even though v2 renamed the node.
  await engine.completeTask({
    taskId: taskByName(fake, 'Review', a.processInstanceId).id,
    userId: 'sme',
    transitionName: 'ok',
  })
  assert.ok(hasTask(fake, 'Legacy Check', a.processInstanceId))
})

// ---------------------------------------------------------------------------
// Rollback-as-new-version + all versions remain available
// ---------------------------------------------------------------------------

test('rollback deploys a prior graph as a NEW version; every version remains available', async () => {
  const { fake, engine, id1, id3 } = setup(true)
  assert.ok(id3)

  const rollback = await engine.startProcess({ definitionKey: KEY, version: 3, startedBy: 'broker' })
  const pi = await engine.getProcessInstance(rollback.processInstanceId)
  assert.equal(pi!.definitionId, id3)
  assert.notEqual(id3, id1, 'rollback is a NEW definition row — never a mutation of v1')

  // The rollback version runs the v1 graph (Review, then Legacy Check on the
  // v1 path — a node that v2 removed).
  assert.ok(hasTask(fake, 'Review', rollback.processInstanceId))
  await engine.completeTask({
    taskId: taskByName(fake, 'Review', rollback.processInstanceId).id,
    userId: 'sme',
    transitionName: 'ok',
  })
  assert.ok(hasTask(fake, 'Legacy Check', rollback.processInstanceId), 'v3 runs the v1 graph')

  // v1 (and v2) remain deployable/available alongside v3.
  const v1 = await engine.startProcess({ definitionKey: KEY, version: 1, startedBy: 'broker' })
  assert.equal((await engine.getProcessInstance(v1.processInstanceId))!.definitionId, id1)
  assert.deepEqual(
    fake.store.processDefinitions.map((d) => d.version).sort(),
    [1, 2, 3],
    'all deployed versions remain available',
  )
})

test('an explicit version is honored even when a newer version exists', async () => {
  const { engine, id1, id2, id3 } = setup(true)
  for (const [version, expectedId] of [
    [1, id1],
    [2, id2],
    [3, id3!],
  ] as const) {
    const started = await engine.startProcess({
      definitionKey: KEY,
      version,
      startedBy: 'broker',
    })
    const pi = await engine.getProcessInstance(started.processInstanceId)
    assert.equal(pi!.definitionId, expectedId, `version ${version} must resolve to its own row`)
  }
})
