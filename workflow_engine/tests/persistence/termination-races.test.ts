import { test } from 'node:test'
import assert from 'node:assert/strict'

import { assertEngineSchema, PersistenceFixture } from './harness'

// ---------------------------------------------------------------------------
// ENG-11 — Cancellation / Termination / Abandonment Semantics.
//
// Central invariant: NO work created before termination may later resurrect
// or advance a terminal instance. These are REAL overlapping-transaction
// proofs on PostgreSQL: two transactions genuinely overlap (row locks held
// while the peer blocks), and the surviving state must be terminal with no
// actionable work — no active tokens, no open tasks, no pending/locked jobs,
// no duplicate terminal events.
//
// Deterministic interleavings use engine hooks as barriers: the parked
// transaction holds its row locks, the racing transaction blocks on them
// (a real lock wait, not a fake), then the gate opens.
// ---------------------------------------------------------------------------

// start -> fork(A, B required) -> join -> end
const FORK_JOIN = {
  startNodeId: 'start',
  nodes: {
    start: {
      id: 'start',
      type: 'start',
      transitions: [{ name: 'go', to: 'fork' }],
    },
    fork: {
      id: 'fork',
      type: 'fork',
      transitions: [
        { name: 'a', to: 'tA', required: true },
        { name: 'b', to: 'tB', required: true },
      ],
    },
    tA: { id: 'tA', type: 'task', name: 'A', transitions: [{ name: 'done', to: 'join' }] },
    tB: { id: 'tB', type: 'task', name: 'B', transitions: [{ name: 'done', to: 'join' }] },
    join: { id: 'join', type: 'join', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

// start -> approve (human task) -> end
const LINEAR_TASK = {
  startNodeId: 'start',
  nodes: {
    start: {
      id: 'start',
      type: 'start',
      transitions: [{ name: 'go', to: 'approve' }],
    },
    approve: {
      id: 'approve',
      type: 'task',
      name: 'Approve',
      transitions: [{ name: 'done', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  },
}

// start -> wait (timer, past due) -> end
const TIMER_END = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'wait' }] },
    wait: {
      id: 'wait',
      type: 'timer',
      transitions: [{ name: 'fire', to: 'end' }],
      timer: { dueAt: '2026-01-01T00:00:00.000Z' },
    },
    end: { id: 'end', type: 'end' },
  },
}

// start -> approve (human task) -> cmd (application command) -> end
const COMMAND_LINE = {
  startNodeId: 'start',
  nodes: {
    start: {
      id: 'start',
      type: 'start',
      transitions: [{ name: 'go', to: 'approve' }],
    },
    approve: {
      id: 'approve',
      type: 'task',
      name: 'Approve',
      transitions: [{ name: 'done', to: 'cmd' }],
    },
    cmd: {
      id: 'cmd',
      type: 'command',
      commandType: 'tunit.conflict',
      transitions: [{ name: 'go', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  },
}

const T0 = new Date('2026-08-21T00:00:00.000Z')

/**
 * Two-phase hook gate: `entered` resolves when the hook fires (the parked
 * transaction now holds its row locks); `open()` releases it. Lets a test
 * force a deterministic overlapping-transaction interleaving.
 */
class HookGate {
  private enteredResolve!: () => void
  private releaseResolve!: () => void
  readonly entered: Promise<void>
  private readonly release: Promise<void>

  constructor() {
    this.entered = new Promise((r) => {
      this.enteredResolve = r
    })
    this.release = new Promise((r) => {
      this.releaseResolve = r
    })
  }

  async hold(): Promise<void> {
    this.enteredResolve()
    await this.release
  }

  open(): void {
    this.releaseResolve()
  }
}

/**
 * A race rejection is acceptable when it is one of the engine's deterministic
 * conflicts — either PROCESS_NOT_ACTIVE (a terminal instance) or the plain
 * task/token guards that fire when termination won (obsoleted task, closed
 * token). Any other rejection would be a genuine engine defect.
 */
function isDeterministicRaceRejection(r: PromiseSettledResult<any>): boolean {
  if (r.status === 'fulfilled') return true
  const code = r.reason?.code
  if (code === 'PROCESS_NOT_ACTIVE') return true
  const msg = String(r.reason?.message ?? r.reason)
  return (
    msg.startsWith('Task cannot be completed in status:') ||
    msg.startsWith('Linked token is not active') ||
    msg.includes(' is not active')
  )
}

/** Terminal instances retain no actionable work: no active tokens, no open
 * tasks, no pending/locked jobs. */
async function assertNoActionableWork(
  f: PersistenceFixture,
  processInstanceId: string,
): Promise<void> {
  const tokens = await f.tokens(processInstanceId)
  for (const t of tokens as Array<{ id: string; status: string }>) {
    assert.equal(
      t.status,
      'completed',
      `token ${t.id} must not be active on a terminal instance (status=${t.status})`,
    )
  }
  const tasks = await f.tasks(processInstanceId)
  for (const t of tasks as Array<{ id: string; status: string }>) {
    assert.ok(
      !['ready', 'reserved', 'in_progress'].includes(t.status),
      `task ${t.id} must not be actionable on a terminal instance (status=${t.status})`,
    )
  }
  const jobs = await f.jobs(processInstanceId)
  for (const j of jobs as Array<{ id: string; status: string }>) {
    assert.ok(
      !['pending', 'locked'].includes(j.status),
      `job ${j.id} must not be actionable on a terminal instance (status=${j.status})`,
    )
  }
}

async function startForkJoin(f: PersistenceFixture, engine: any) {
  return engine.startProcess({
    definitionKey: 'tunit_term',
    version: 1,
    tenantId: f.tenantId,
    startedBy: 'tester',
  })
}

// ---------------------------------------------------------------------------
// Idempotent cancellation
// ---------------------------------------------------------------------------

test('ENG-11: cancelProcess is idempotent and leaves no actionable work', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, FORK_JOIN)
    const engine = f.makeEngine()
    const { processInstanceId } = await startForkJoin(f, engine)

    const tasks = (await f.tasks(processInstanceId)) as Array<{ id: string }>
    await engine.claimTask(tasks[0].id, 'u')

    await engine.cancelProcess({ processInstanceId, actor: 'boss', reason: 'client withdrew' })

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'aborted')
    assert.equal(inst.outcome, 'cancelled')

    await assertNoActionableWork(f, processInstanceId)

    // Human task list no longer surfaces the obsoleted task.
    const active = await engine.getActiveTasksForUser('u', f.tenantId)
    assert.equal(
      active.some((t: any) => t.processInstanceId === processInstanceId),
      false,
      'obsoleted task must not surface in the active task list',
    )

    // Second cancel: idempotent no-throw; exactly one terminal event.
    await engine.cancelProcess({ processInstanceId, actor: 'boss' })
    const cancelledEvents = await f.events(processInstanceId, 'process.cancelled')
    assert.equal(cancelledEvents.length, 1, 'exactly one process.cancelled event')
    const obsoleted = await f.events(processInstanceId, 'task.obsoleted')
    assert.equal(obsoleted.length, 2, 'both open tasks obsoleted exactly once')
  } finally {
    await f.cleanup()
  }
})

// ---------------------------------------------------------------------------
// cancel-vs-join
// ---------------------------------------------------------------------------

test('ENG-11: cancel-vs-join — cancellation wins, the join never releases (no resurrection)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, FORK_JOIN)
    const gate = new HookGate()
    const engine = f.makeEngine({
      hooks: {
        afterJoinParentLock: async () => {
          await gate.hold()
        },
      },
    })
    const { processInstanceId } = await startForkJoin(f, engine)
    const tasks = (await f.tasks(processInstanceId)) as Array<{ id: string; name: string }>
    const taskA = tasks.find((t) => t.name === 'A')!

    // Branch A completion parks at the join parent lock, HOLDING the instance,
    // task-A, token-A and parent-token row locks. The cancellation therefore
    // blocks on the instance row — a real overlapping transaction.
    const completing = engine.completeTask({ taskId: taskA.id, userId: 'u', transitionName: 'done' })
    await gate.entered

    const cancelling = engine.cancelProcess({ processInstanceId, actor: 'boss' })
    gate.open()

    const [completeResult, cancelResult] = await Promise.allSettled([completing, cancelling])
    assert.equal(completeResult.status, 'fulfilled', 'branch completion settles cleanly (no deadlock)')
    assert.equal(cancelResult.status, 'fulfilled', 'cancellation settles cleanly (no deadlock)')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'aborted')
    assert.equal(inst.outcome, 'cancelled')

    const joined = await f.events(processInstanceId, 'token.joined')
    assert.equal(joined.length, 0, 'join must never release on a cancelled process')

    await assertNoActionableWork(f, processInstanceId)

    // The surviving branch token was abandoned (closed cancelled); its task
    // was obsoleted.
    const tokens = await f.tokens(processInstanceId)
    const branchB = tokens.find((t: any) => t.node_id === 'tB')
    assert.equal(branchB.status, 'completed')
    assert.equal(branchB.outcome, 'cancelled')
    const taskB = (await f.tasks(processInstanceId)).find((t: any) => t.name === 'B')
    assert.equal(taskB.status, 'obsolete')
  } finally {
    await f.cleanup()
  }
})

test('ENG-11: cancel-vs-join — advance wins, then cancel is rejected (no resurrection)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, FORK_JOIN)
    const gate = new HookGate()
    const engine = f.makeEngine({
      hooks: {
        beforeNodeArrive: async (nodeId: string) => {
          if (nodeId === 'end') await gate.hold()
        },
      },
    })
    const { processInstanceId } = await startForkJoin(f, engine)
    const tasks = (await f.tasks(processInstanceId)) as Array<{ id: string; name: string }>
    const taskA = tasks.find((t) => t.name === 'A')!
    const taskB = tasks.find((t) => t.name === 'B')!

    // Complete A (join waits), then park B's completion AFTER the join
    // released and the successor arrived at the end node — the instance lock
    // is held and the process is about to complete.
    await engine.completeTask({ taskId: taskA.id, userId: 'u', transitionName: 'done' })
    const completing = engine.completeTask({ taskId: taskB.id, userId: 'u', transitionName: 'done' })
    await gate.entered

    const cancelling = engine.cancelProcess({ processInstanceId, actor: 'boss' })
    gate.open()

    const [completeResult, cancelResult] = await Promise.allSettled([completing, cancelling])
    assert.equal(completeResult.status, 'fulfilled', 'join release completes the process')
    assert.equal(cancelResult.status, 'rejected')
    assert.equal((cancelResult as any).reason?.code, 'PROCESS_NOT_ACTIVE')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')
    assert.equal(inst.outcome, 'completed')

    const joined = await f.events(processInstanceId, 'token.joined')
    assert.equal(joined.length, 1, 'exactly one join release')
    const completedEvents = await f.events(processInstanceId, 'process.completed')
    assert.equal(completedEvents.length, 1, 'exactly one process.completed event')
    const cancelledEvents = await f.events(processInstanceId, 'process.cancelled')
    assert.equal(cancelledEvents.length, 0, 'no process.cancelled on a completed process')

    await assertNoActionableWork(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-11: cancel-vs-join — concurrent stress (either order, invariant holds)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, FORK_JOIN)
    const engine = f.makeEngine()

    for (let i = 0; i < 4; i++) {
      const { processInstanceId } = await startForkJoin(f, engine)
      const tasks = (await f.tasks(processInstanceId)) as Array<{ id: string }>

      const results = await Promise.allSettled([
        engine.completeTask({ taskId: tasks[0].id, userId: 'u', transitionName: 'done' }),
        engine.completeTask({ taskId: tasks[1].id, userId: 'u', transitionName: 'done' }),
        engine.cancelProcess({ processInstanceId, actor: 'boss' }),
      ])
      assert.ok(
        results.every(isDeterministicRaceRejection),
        `iteration ${i}: unexpected rejection ${JSON.stringify(
          results.filter((r) => !isDeterministicRaceRejection(r)).map((r: any) => r.reason?.message ?? r.reason),
        )}`,
      )

      const inst = await f.instance(processInstanceId)
      assert.ok(
        inst.status === 'aborted' || inst.status === 'completed',
        `iteration ${i}: instance must be terminal (status=${inst.status})`,
      )
      const joined = await f.events(processInstanceId, 'token.joined')
      assert.ok(joined.length <= 1, `iteration ${i}: at most one join release`)
      await assertNoActionableWork(f, processInstanceId)
    }
  } finally {
    await f.cleanup()
  }
})

// ---------------------------------------------------------------------------
// cancel-vs-human-complete
// ---------------------------------------------------------------------------

test('ENG-11: cancel-vs-human-complete — cancellation wins, completion is rejected', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, LINEAR_TASK)
    const gate = new HookGate()
    const engine = f.makeEngine({
      hooks: {
        beforeProcessTerminal: async () => {
          await gate.hold()
        },
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_term',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }

    // Cancellation parks inside the terminal transition, HOLDING the instance
    // lock; the human completion blocks on it.
    const cancelling = engine.cancelProcess({ processInstanceId, actor: 'boss' })
    await gate.entered

    const completing = engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' })
    gate.open()

    const [cancelResult, completeResult] = await Promise.allSettled([cancelling, completing])
    assert.equal(cancelResult.status, 'fulfilled')
    assert.equal(completeResult.status, 'rejected', 'completion must fail on a cancelled process')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'aborted')
    assert.equal(inst.outcome, 'cancelled')

    await assertNoActionableWork(f, processInstanceId)
    const tasks = await f.tasks(processInstanceId)
    assert.equal(tasks[0].status, 'obsolete', 'the human task is obsoleted, never completed')
    const completedEvents = await f.events(processInstanceId, 'task.completed')
    assert.equal(completedEvents.length, 0, 'no task.completed after cancellation')
  } finally {
    await f.cleanup()
  }
})

test('ENG-11: cancel-vs-human-complete — advance wins, then cancel is rejected', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, LINEAR_TASK)
    const gate = new HookGate()
    const engine = f.makeEngine({
      hooks: {
        beforeNodeArrive: async (nodeId: string) => {
          if (nodeId === 'end') await gate.hold()
        },
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_term',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }

    const completing = engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' })
    await gate.entered

    const cancelling = engine.cancelProcess({ processInstanceId, actor: 'boss' })
    gate.open()

    const [completeResult, cancelResult] = await Promise.allSettled([completing, cancelling])
    assert.equal(completeResult.status, 'fulfilled')
    assert.equal(cancelResult.status, 'rejected')
    assert.equal((cancelResult as any).reason?.code, 'PROCESS_NOT_ACTIVE')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')
    assert.equal(inst.outcome, 'completed')
    await assertNoActionableWork(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-11: cancel-vs-human-complete — concurrent stress (either order, invariant holds)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, LINEAR_TASK)
    const engine = f.makeEngine()

    for (let i = 0; i < 4; i++) {
      const { processInstanceId } = await engine.startProcess({
        definitionKey: 'tunit_term',
        version: 1,
        tenantId: f.tenantId,
        startedBy: 'tester',
      })
      const task = (await f.tasks(processInstanceId))[0] as { id: string }

      const results = await Promise.allSettled([
        engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' }),
        engine.cancelProcess({ processInstanceId, actor: 'boss' }),
      ])
      assert.ok(
        results.every(isDeterministicRaceRejection),
        `iteration ${i}: unexpected rejection ${JSON.stringify(
          results.filter((r) => !isDeterministicRaceRejection(r)).map((r: any) => r.reason?.message ?? r.reason),
        )}`,
      )

      const inst = await f.instance(processInstanceId)
      assert.ok(
        inst.status === 'aborted' || inst.status === 'completed',
        `iteration ${i}: instance must be terminal (status=${inst.status})`,
      )
      await assertNoActionableWork(f, processInstanceId)
    }
  } finally {
    await f.cleanup()
  }
})

// ---------------------------------------------------------------------------
// cancel-vs-job-claim + timer-after-cancel
// ---------------------------------------------------------------------------

test('ENG-11: cancel-vs-job-claim — claimed job is cancelled by termination; firing cannot advance', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_term',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const job = (await f.jobs(processInstanceId))[0] as { id: string }

    // A worker claims the job; the operator then cancels the process.
    const claimed = await engine.claimJobs('worker', 10)
    assert.equal(claimed.length, 1)
    assert.equal(claimed[0].status, 'locked')

    await engine.cancelProcess({ processInstanceId, actor: 'boss' })

    const jobs = await f.jobs(processInstanceId)
    assert.equal(jobs[0].status, 'cancelled', 'the claimed job is cancelled by termination')
    assert.equal(jobs[0].locked_by, null, 'cancellation releases the worker lock')

    // Firing the now-cancelled job is rejected deterministically...
    await assert.rejects(
      () => engine.fireTimerJob({ jobId: job.id, workerId: 'worker' }),
      (e: any) => e.code === 'TIMER_NOT_LOCKED',
    )

    // ...and the poller survives it: nothing is claimed, nothing fires,
    // nothing fails, and the job is never re-opened.
    const res = await engine.runDueJobs('poller', 10)
    assert.equal(res.claimed.length, 0)
    assert.equal(res.fired, 0)
    assert.equal(res.failed, 0)

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'aborted')
    assert.equal(inst.outcome, 'cancelled')
    await assertNoActionableWork(f, processInstanceId)

    const tokens = await f.tokens(processInstanceId)
    assert.equal(tokens[0].node_id, 'wait', 'the timer token never moved after cancellation')
    assert.equal(tokens[0].outcome, 'cancelled')
  } finally {
    await f.cleanup()
  }
})

test('ENG-11: timer-after-cancel — nothing to claim or fire after termination', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_term',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })

    await engine.cancelProcess({ processInstanceId, actor: 'boss' })

    // No due pending work survives termination.
    const claimed = await engine.claimJobs('worker', 10)
    assert.equal(claimed.length, 0, 'a cancelled job is not claimable')
    const overdue = await engine.listOverdueJobs(50)
    assert.equal(
      overdue.some((j: any) => j.processInstanceId === processInstanceId),
      false,
      'no overdue pending job for the terminal instance',
    )

    const res = await engine.runDueJobs('poller', 10)
    assert.equal(res.fired, 0)
    assert.equal(res.failed, 0)

    await assertNoActionableWork(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-11: timer-after-cancel — fire-first advance is rejected by cancellation (no resurrection)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_term',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const job = (await f.jobs(processInstanceId))[0] as { id: string }

    // The timer fires first and the process completes.
    await engine.claimJobs('worker', 10)
    await engine.fireTimerJob({ jobId: job.id, workerId: 'worker' })
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')

    // Cancellation of a completed process is a deterministic conflict.
    await assert.rejects(
      () => engine.cancelProcess({ processInstanceId, actor: 'boss' }),
      (e: any) => e.code === 'PROCESS_NOT_ACTIVE',
    )

    await assertNoActionableWork(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-11: cancel-vs-job-claim — concurrent stress (either order, no pending/locked residue)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })

    for (let i = 0; i < 4; i++) {
      const { processInstanceId } = await engine.startProcess({
        definitionKey: 'tunit_term',
        version: 1,
        tenantId: f.tenantId,
        startedBy: 'tester',
      })

      await Promise.allSettled([
        engine.claimJobs('worker', 10),
        engine.cancelProcess({ processInstanceId, actor: 'boss' }),
      ])

      const inst = await f.instance(processInstanceId)
      assert.equal(inst.status, 'aborted', `iteration ${i}: cancellation always terminates`)
      assert.equal(inst.outcome, 'cancelled')
      await assertNoActionableWork(f, processInstanceId)
    }
  } finally {
    await f.cleanup()
  }
})

// ---------------------------------------------------------------------------
// terminal-failure-vs-explicit-cancel
// ---------------------------------------------------------------------------

test('ENG-11: terminal-failure-vs-cancel — command conflict wins, cancel is rejected', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, COMMAND_LINE)
    const gate = new HookGate()
    const engine = f.makeEngine({
      app: {
        executeCommand: async (req: any) => ({
          commandId: req.commandId,
          outcome: 'conflict' as const,
          message: 'terminal conflict',
        }),
        readFacts: async () => ({}),
      },
      hooks: {
        afterCommandSideEffect: async () => {
          await gate.hold()
        },
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_term',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }

    // The terminal failure is mid-flight (instance lock held, app side effect
    // already committed); the explicit cancel blocks on the instance row.
    const completing = engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' })
    await gate.entered

    const cancelling = engine.cancelProcess({ processInstanceId, actor: 'boss' })
    gate.open()

    const [completeResult, cancelResult] = await Promise.allSettled([completing, cancelling])
    assert.equal(completeResult.status, 'fulfilled', 'terminal failure settles cleanly (no deadlock)')
    assert.equal(cancelResult.status, 'rejected')
    assert.equal((cancelResult as any).reason?.code, 'PROCESS_NOT_ACTIVE')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'error')
    assert.equal(inst.outcome, 'conflict')
    const conflictEvents = await f.events(processInstanceId, 'process.conflict')
    assert.equal(conflictEvents.length, 1, 'exactly one process.conflict event')
    const cancelledEvents = await f.events(processInstanceId, 'process.cancelled')
    assert.equal(cancelledEvents.length, 0, 'no process.cancelled after terminal failure')

    const cmds = await f.commands(processInstanceId)
    assert.equal(cmds.length, 1)
    assert.equal(cmds[0].outcome, 'conflict')

    await assertNoActionableWork(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-11: terminal-failure-vs-cancel — cancel wins, the command never executes', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, COMMAND_LINE)
    const gate = new HookGate()
    const engine = f.makeEngine({
      app: {
        executeCommand: async (req: any) => {
          throw new Error('executeCommand must not run after cancellation')
        },
        readFacts: async () => ({}),
      },
      hooks: {
        beforeProcessTerminal: async () => {
          await gate.hold()
        },
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_term',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }

    // Cancellation parks mid-terminal-transition (instance lock held); the
    // human completion — which would drive the terminal-failing command —
    // blocks on the instance row.
    const cancelling = engine.cancelProcess({ processInstanceId, actor: 'boss' })
    await gate.entered

    const completing = engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' })
    gate.open()

    const [cancelResult, completeResult] = await Promise.allSettled([cancelling, completing])
    assert.equal(cancelResult.status, 'fulfilled')
    assert.equal(completeResult.status, 'rejected', 'completion must fail on a cancelled process')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'aborted')
    assert.equal(inst.outcome, 'cancelled')

    const cmds = await f.commands(processInstanceId)
    assert.equal(cmds.length, 0, 'the terminal-failing command never ran on the cancelled process')
    const app = (engine as any).app
    assert.equal(app.calls.length, 0, 'the application port was never invoked after cancellation')

    await assertNoActionableWork(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-11: terminal-failure-vs-cancel — concurrent stress (either order, invariant holds)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, COMMAND_LINE)
    const engine = f.makeEngine({
      app: {
        executeCommand: async (req: any) => ({
          commandId: req.commandId,
          outcome: 'conflict' as const,
          message: 'terminal conflict',
        }),
        readFacts: async () => ({}),
      },
    })

    for (let i = 0; i < 3; i++) {
      const { processInstanceId } = await engine.startProcess({
        definitionKey: 'tunit_term',
        version: 1,
        tenantId: f.tenantId,
        startedBy: 'tester',
      })
      const task = (await f.tasks(processInstanceId))[0] as { id: string }

      const results = await Promise.allSettled([
        engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' }),
        engine.cancelProcess({ processInstanceId, actor: 'boss' }),
      ])
      assert.ok(
        results.every(isDeterministicRaceRejection),
        `iteration ${i}: unexpected rejection ${JSON.stringify(
          results.filter((r) => !isDeterministicRaceRejection(r)).map((r: any) => r.reason?.message ?? r.reason),
        )}`,
      )

      const inst = await f.instance(processInstanceId)
      assert.ok(
        inst.status === 'aborted' || inst.status === 'error',
        `iteration ${i}: instance must be terminal (status=${inst.status})`,
      )
      const terminalEvents =
        (await f.events(processInstanceId, 'process.cancelled')).length +
        (await f.events(processInstanceId, 'process.conflict')).length
      assert.equal(terminalEvents, 1, `iteration ${i}: exactly one terminal event`)
      await assertNoActionableWork(f, processInstanceId)
    }
  } finally {
    await f.cleanup()
  }
})

// ---------------------------------------------------------------------------
// No work creation on terminal instances (abandonment closure)
// ---------------------------------------------------------------------------

test('ENG-11: a failed job on a cancelled instance cannot be requeued (no resurrected work)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, TIMER_END)
    let nowMs = T0.getTime() + 10_000
    const engine = f.makeEngine({ now: () => new Date(nowMs) })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_term',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const jobId = (await f.jobs(processInstanceId))[0].id

    // Exhaust the job to terminal 'failed'.
    for (let i = 0; i < 5; i++) {
      const claimed = await engine.claimJobs('w', 1)
      assert.equal(claimed.length, 1, `iteration ${i}: expected one claim`)
      await engine.failJob(jobId, 'w', `boom ${i}`)
      nowMs += 30 * 60_000
    }
    let job = await engine.getJob(jobId)
    assert.equal(job!.status, 'failed')

    // Now the process is cancelled; the failed job must never become
    // actionable again.
    await engine.cancelProcess({ processInstanceId, actor: 'boss' })

    await assert.rejects(
      () => engine.requeueJob({ jobId, actor: 'operator' }),
      (e: any) => e.code === 'JOB_NOT_REQUEUEABLE',
    )

    job = await engine.getJob(jobId)
    assert.equal(job!.status, 'failed', 'the failed job stays settled')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'aborted')
    assert.equal(inst.outcome, 'cancelled')
    await assertNoActionableWork(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-11: a job cannot be created on a terminal instance (no new actionable work)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_term', 1, LINEAR_TASK)
    const engine = f.makeEngine()
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_term',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })

    await engine.cancelProcess({ processInstanceId, actor: 'boss' })

    await assert.rejects(
      () =>
        engine.createJob({
          processInstanceId,
          tenantId: f.tenantId,
          type: 'async',
          dueAt: new Date(),
        }),
      (e: any) => e.code === 'PROCESS_NOT_ACTIVE',
    )

    const jobs = await f.jobs(processInstanceId)
    assert.equal(jobs.length, 0, 'no job row was created on the terminal instance')
  } finally {
    await f.cleanup()
  }
})
