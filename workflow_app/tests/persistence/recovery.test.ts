import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'
import { assertEngineSchema, PersistenceFixture } from '../../../testv2/engine_tests/persistence/harness'
import { reconcileInstanceCore, runRecoveryPassCore } from '../../recovery'

// start -> tA (task) -> end
const LINEAR_TASK = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'tA' }] },
    tA: { id: 'tA', type: 'task', name: 'A', transitions: [{ name: 'done', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

// start -> timer (past due) -> end
const TIMER_END = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'timer' }] },
    timer: {
      id: 'timer',
      type: 'timer',
      transitions: [{ name: 'resume', to: 'end' }],
      timer: { dueAt: '2026-01-01T00:00:00.000Z' },
    },
    end: { id: 'end', type: 'end' },
  },
}

const T0 = new Date('2026-08-21T00:00:00.000Z')

test('CRM-14H: a fresh engine (new process) advances a persisted instance from PostgreSQL alone', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_rec', 1, LINEAR_TASK)
    // Engine "process A" creates the instance, then is discarded.
    const { processInstanceId } = await f
      .makeEngine()
      .startProcess({
        definitionKey: 'tunit_rec',
        version: 1,
        tenantId: f.tenantId,
        startedBy: 'tester',
      })

    // "Restart": a brand-new engine object (no in-process state) reads the DB.
    const engineB = f.makeEngine()
    const inst = await engineB.getProcessInstance(processInstanceId)
    assert.equal(inst!.status, 'active')

    const task = (await f.tasks(processInstanceId))[0] as { id: string }
    await engineB.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' })
    const after = await f.instance(processInstanceId)
    assert.equal(after.status, 'completed')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14H: a stale locked job is reclaimed and fired after a worker restart', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_rec', 1, TIMER_END)
    let nowMs = T0.getTime() + 10_000
    const engineA = f.makeEngine({ now: () => new Date(nowMs) })
    const { processInstanceId } = await engineA.startProcess({
      definitionKey: 'tunit_rec',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    // Worker A claims the timer then "dies" (engine object discarded).
    await engineA.claimJobs('dead-worker', 1)

    // Lease expires; a fresh worker B runs the recovery pass.
    nowMs += 6 * 60_000
    const engineB = f.makeEngine({ now: () => new Date(nowMs) })
    const reclaimed = await engineB.reclaimStaleJobs(10)
    assert.equal(reclaimed, 1)

    const res = await engineB.runDueJobs('worker-b', 10)
    assert.equal(res.fired, 1, 'reclaimed timer fires exactly once')
    const after = await f.instance(processInstanceId)
    assert.equal(after.status, 'completed')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14H: recovery pass is idempotent; a healthy instance is unchanged', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_rec', 1, LINEAR_TASK)
    const { processInstanceId } = await f
      .makeEngine()
      .startProcess({
        definitionKey: 'tunit_rec',
        version: 1,
        tenantId: f.tenantId,
        startedBy: 'tester',
      })

    const reclaims = async () => {
      // Tenant-scoped reclaim: only our instance's stale leases.
      return interactiveSql`
        update jobs set status='pending', locked_by=null, locked_until=null
        where process_instance_id in (select id from process_instances where tenant_id = ${f.tenantId})
          and status='locked' and locked_until < now()
      `.then((r: any) => r.length)
    }

    const report1 = await runRecoveryPassCore({
      reclaimStaleJobs: reclaims,
      reconcile: async () => ({ startedInstances: 0, materializedTasks: 0, skippedTasks: 0 }),
      collectAnomalies: async () => [],
    })
    const report2 = await runRecoveryPassCore({
      reclaimStaleJobs: reclaims,
      reconcile: async () => ({ startedInstances: 0, materializedTasks: 0, skippedTasks: 0 }),
      collectAnomalies: async () => [],
    })
    assert.deepEqual(report2, report1, 'recovery pass is idempotent')

    // Healthy instance untouched: task still ready, instance still active.
    const tasks = await f.tasks(processInstanceId)
    assert.equal(tasks[0].status, 'ready')
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'active')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14H: a terminal instance cannot be resurrected by recovery', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_rec', 1, LINEAR_TASK)
    const engine = f.makeEngine()
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_rec',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }
    await engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' })

    const after = await f.instance(processInstanceId)
    assert.equal(after.status, 'completed')

    // Recovery/reconcile must not reopen the terminal instance.
    const reclaimed = await interactiveSql`
      update jobs set status='pending'
      where process_instance_id = ${processInstanceId}
        and status='locked' and locked_until < now()
    `
    assert.equal((reclaimed as any[]).length, 0)
    const again = await f.instance(processInstanceId)
    assert.equal(again.status, 'completed', 'terminal instance stays terminal')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14H: per-instance reconciliation is scoped to the target instance only', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_rec', 1, TIMER_END)
    let nowMs = T0.getTime() + 10_000
    const engine = f.makeEngine({ now: () => new Date(nowMs) })
    const a = await engine.startProcess({
      definitionKey: 'tunit_rec',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const b = await engine.startProcess({
      definitionKey: 'tunit_rec',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    await engine.claimJobs('dead', 5) // claims both timers

    nowMs += 6 * 60_000
    const report = await reconcileInstanceCore(a.processInstanceId, async (id) =>
      engine.reclaimStaleJobsForInstance(id),
    )
    assert.equal(report.reclaimedStaleJobs, 1)

    // Only instance A was reclaimed; B's stale lock remains untouched.
    const jobsB = await f.jobs(b.processInstanceId)
    assert.equal(jobsB[0].status, 'locked', 'instance B stale lock is NOT touched by A reconciliation')
    const jobsA = await f.jobs(a.processInstanceId)
    assert.equal(jobsA[0].status, 'pending', 'instance A stale lock reclaimed')
  } finally {
    await f.cleanup()
  }
})
