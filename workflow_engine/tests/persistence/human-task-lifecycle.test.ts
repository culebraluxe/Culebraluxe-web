import { test } from 'node:test'
import assert from 'node:assert/strict'

import { assertEngineSchema, PersistenceFixture } from './harness'

// ---------------------------------------------------------------------------
// ENG-13 — Generic Human Task Semantics (real PostgreSQL proofs).
//
// Formalized persisted human-task lifecycle:
//   ready -> reserved (claim / reassign) -> completed
//            reserved -> ready (release)
//   any open state -> obsolete (terminal cleanup on process termination)
//
// The engine owns the task runtime/assignment mechanics (locking, version
// guards, audit events); identity/authorization stay in the application layer
// (the engine only records the actor strings it is handed). These tests prove,
// on real PostgreSQL with genuinely overlapping transactions, that:
//   - claim / release / reassign / complete behave deterministically,
//   - duplicate completion is a deterministic conflict and the token advances
//     EXACTLY ONCE,
//   - claim-vs-complete and completion-vs-cancellation races never produce a
//     double advance, a resurrected instance, or actionable residue,
//   - after a terminal process state there are NO actionable tasks and every
//     task operation is a deterministic conflict.
// ---------------------------------------------------------------------------

// start -> review (human task, candidates [u1, u2]) -> end
const LINEAR_TASK = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'review' }] },
    review: {
      id: 'review',
      type: 'task',
      name: 'Review',
      candidateGroups: ['u1', 'u2'],
      transitions: [{ name: 'done', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  },
}

// start -> fork(tA, tB required) -> join -> end
const FORK_TASKS = {
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
    tA: {
      id: 'tA',
      type: 'task',
      name: 'A',
      candidateGroups: ['u1', 'u2'],
      transitions: [{ name: 'done', to: 'join' }],
    },
    tB: {
      id: 'tB',
      type: 'task',
      name: 'B',
      candidateGroups: ['u1', 'u2'],
      transitions: [{ name: 'done', to: 'join' }],
    },
    join: { id: 'join', type: 'join', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

async function startLinear(f: PersistenceFixture, engine: any) {
  return engine.startProcess({
    definitionKey: 'tunit_ht',
    version: 1,
    tenantId: f.tenantId,
    startedBy: 'tester',
  })
}

async function startFork(f: PersistenceFixture, engine: any) {
  return engine.startProcess({
    definitionKey: 'tunit_ht',
    version: 1,
    tenantId: f.tenantId,
    startedBy: 'tester',
  })
}

/** Count token advances that leave the given task node(s) — the token.moved
 * events whose `from` is the task node. (The engine also records a token.moved
 * for the start->first-node hop, so counting ALL token.moved events would
 * over-count; the task-node advance is the exactly-once signal.) */
async function taskAdvanceCount(
  f: PersistenceFixture,
  processInstanceId: string,
  taskNodeIds: string[],
): Promise<number> {
  const moved = (await f.events(processInstanceId, 'token.moved')) as Array<{
    data: { from?: string }
  }>
  return moved.filter((e) => taskNodeIds.includes(e.data?.from ?? '')).length
}

/** Exactly-once completion: one task.completed, one task-node token advance,
 * one completed process, the token resting on the end node. */
async function assertExactlyOnceCompletion(
  f: PersistenceFixture,
  processInstanceId: string,
  taskNodeId = 'review',
): Promise<void> {
  const completedEvents = await f.events(processInstanceId, 'task.completed')
  assert.equal(completedEvents.length, 1, 'exactly one task.completed event')
  const advances = await taskAdvanceCount(f, processInstanceId, [taskNodeId])
  assert.equal(advances, 1, 'exactly one token advance past the task node')
  const processCompleted = await f.events(processInstanceId, 'process.completed')
  assert.equal(processCompleted.length, 1, 'exactly one process.completed event')

  const tokens = await f.tokens(processInstanceId)
  const endTokens = tokens.filter((t: any) => t.node_id === 'end' && t.status === 'completed')
  assert.equal(endTokens.length, 1, 'exactly one completed token on the end node')

  const inst = await f.instance(processInstanceId)
  assert.equal(inst!.status, 'completed')
  assert.equal(inst!.outcome, 'completed')
}

/** No task on the instance may be actionable (ready/reserved/in_progress). */
async function assertNoActionableTasks(
  f: PersistenceFixture,
  processInstanceId: string,
): Promise<void> {
  const tasks = (await f.tasks(processInstanceId)) as Array<{ id: string; status: string }>
  for (const t of tasks) {
    assert.ok(
      !['ready', 'reserved', 'in_progress'].includes(t.status),
      `task ${t.id} must not be actionable on a terminal instance (status=${t.status})`,
    )
  }
}

/** A race rejection is acceptable only when it is one of the engine's
 * deterministic task-lifecycle conflicts. Anything else is an engine defect. */
function isDeterministicTaskRejection(r: PromiseSettledResult<any>): boolean {
  if (r.status === 'fulfilled') return true
  const code = r.reason?.code
  const deterministic = new Set([
    'TASK_ALREADY_COMPLETED',
    'TASK_NOT_ACTIONABLE',
    'TASK_NOT_CLAIMABLE',
    'TASK_NOT_RELEASABLE',
    'TASK_NOT_REASSIGNABLE',
    'TASK_ASSIGNEE_ONLY',
    'TASK_CANDIDATE_ONLY',
    'TASK_ALREADY_ASSIGNED',
    'PROCESS_NOT_ACTIVE',
    'STALE_TASK',
  ])
  if (deterministic.has(code)) return true
  const msg = String(r.reason?.message ?? r.reason)
  return (
    msg.startsWith('Task cannot be completed in status:') ||
    msg.startsWith('Linked token is not active') ||
    msg.includes(' is not active')
  )
}

/** Two-phase hook gate: `entered` resolves when the hook fires (the parked
 * transaction now holds its row locks); `open()` releases it. Forces a
 * deterministic overlapping-transaction interleaving. */
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

// ---------------------------------------------------------------------------
// claim / release / reassign / complete behavior
// ---------------------------------------------------------------------------

test('ENG-13: claim/release/reassign/complete lifecycle on real PostgreSQL', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const engine = f.makeEngine()
    const { processInstanceId } = await startLinear(f, engine)

    let tasks = (await f.tasks(processInstanceId)) as Array<{
      id: string
      status: string
      assignee: string | null
      candidates: string[]
      version: number
      completed_by: string | null
      completed_at: Date | null
    }>
    assert.equal(tasks.length, 1)
    assert.equal(tasks[0].status, 'ready')
    assert.deepEqual(tasks[0].candidates, ['u1', 'u2'])
    const taskId = tasks[0].id

    // Claim by u1.
    await engine.claimTask(taskId, 'u1')
    tasks = (await f.tasks(processInstanceId)) as typeof tasks
    assert.equal(tasks[0].status, 'reserved')
    assert.equal(tasks[0].assignee, 'u1')
    assert.equal(tasks[0].version, 2, 'claim bumps the task version')
    const claimedEvents = await f.events(processInstanceId, 'task.claimed')
    assert.equal(claimedEvents.length, 1)

    // Only the assignee may release.
    await assert.rejects(
      () => engine.releaseTask(taskId, 'u2'),
      (e: any) => e.code === 'TASK_ASSIGNEE_ONLY',
    )
    await engine.releaseTask(taskId, 'u1')
    tasks = (await f.tasks(processInstanceId)) as typeof tasks
    assert.equal(tasks[0].status, 'ready')
    assert.equal(tasks[0].assignee, null)
    assert.equal((await f.events(processInstanceId, 'task.released')).length, 1)

    // Reassign from u1 (after a fresh claim) to u2, initiated by a manager.
    await engine.claimTask(taskId, 'u1')
    await engine.reassignTask(taskId, 'u2', 'manager')
    tasks = (await f.tasks(processInstanceId)) as typeof tasks
    assert.equal(tasks[0].status, 'reserved')
    assert.equal(tasks[0].assignee, 'u2')
    const reassignedEvents = (await f.events(
      processInstanceId,
      'task.reassigned',
    )) as Array<{ data: { from: string | null; to: string }; actor: string }>
    assert.equal(reassignedEvents.length, 1)
    assert.equal(reassignedEvents[0].data.from, 'u1')
    assert.equal(reassignedEvents[0].data.to, 'u2')
    assert.equal(reassignedEvents[0].actor, 'manager')

    // The displaced user cannot complete; the new assignee completes.
    await assert.rejects(
      () => engine.completeTask({ taskId, userId: 'u1', transitionName: 'done' }),
      (e: any) => e.code === 'TASK_ASSIGNEE_ONLY',
    )
    await engine.completeTask({ taskId, userId: 'u2', transitionName: 'done' })
    tasks = (await f.tasks(processInstanceId)) as typeof tasks
    assert.equal(tasks[0].status, 'completed')
    assert.equal(tasks[0].completed_by, 'u2')
    assert.ok(tasks[0].completed_at, 'completed_at is set')

    await assertExactlyOnceCompletion(f, processInstanceId)
    await assertNoActionableTasks(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-13: reassign pre-assigns a ready task, validates eligibility, refuses completed tasks', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const engine = f.makeEngine()
    const { processInstanceId } = await startLinear(f, engine)
    const taskId = ((await f.tasks(processInstanceId)) as Array<{ id: string }>)[0].id

    // Reassign a ready task = pre-assignment (from null).
    await engine.reassignTask(taskId, 'u2', 'manager')
    let tasks = (await f.tasks(processInstanceId)) as Array<{
      status: string
      assignee: string | null
    }>
    assert.equal(tasks[0].status, 'reserved')
    assert.equal(tasks[0].assignee, 'u2')
    const first = (await f.events(processInstanceId, 'task.reassigned')) as Array<{
      data: { from: string | null; to: string }
    }>
    assert.equal(first[0].data.from, null)
    assert.equal(first[0].data.to, 'u2')

    // Non-candidates are rejected; task stays with u2.
    await assert.rejects(
      () => engine.reassignTask(taskId, 'outsider', 'manager'),
      (e: any) => e.code === 'TASK_CANDIDATE_ONLY',
    )
    tasks = (await f.tasks(processInstanceId)) as typeof tasks
    assert.equal(tasks[0].assignee, 'u2')

    // On a linear process, completing the task completes the process — the
    // instance is terminal, so reassign reports PROCESS_NOT_ACTIVE.
    // (TASK_ALREADY_COMPLETED for reassign is proven on an ACTIVE fork
    // instance in the next test.)
    await engine.completeTask({ taskId, userId: 'u2', transitionName: 'done' })
    await assert.rejects(
      () => engine.reassignTask(taskId, 'u1', 'manager'),
      (e: any) => e.code === 'PROCESS_NOT_ACTIVE',
    )
    await assertExactlyOnceCompletion(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-13: reassign on a completed branch task of an ACTIVE process is TASK_ALREADY_COMPLETED', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, FORK_TASKS)
    const engine = f.makeEngine()
    const { processInstanceId } = await startFork(f, engine)
    const tasks = (await f.tasks(processInstanceId)) as Array<{ id: string; name: string }>
    const taskA = tasks.find((t) => t.name === 'A')!

    // Completing branch A leaves the process ACTIVE (fork waits on B) — so a
    // completed task coexists with an active instance.
    await engine.completeTask({ taskId: taskA.id, userId: 'u1', transitionName: 'done' })
    const inst = await f.instance(processInstanceId)
    assert.equal(inst!.status, 'active')

    await assert.rejects(
      () => engine.reassignTask(taskA.id, 'u2', 'manager'),
      (e: any) => e.code === 'TASK_ALREADY_COMPLETED',
    )
    await assert.rejects(
      () => engine.completeTask({ taskId: taskA.id, userId: 'u1', transitionName: 'done' }),
      (e: any) => e.code === 'TASK_ALREADY_COMPLETED',
    )
    assert.equal((await f.events(processInstanceId, 'task.completed')).length, 1)

    // Branch B remains actionable and completes the process exactly once.
    const taskB = tasks.find((t) => t.name === 'B')!
    await engine.completeTask({ taskId: taskB.id, userId: 'u2', transitionName: 'done' })

    // Fork: each branch token moved once (tA->join, tB->join), both tasks
    // completed, one join release, one process completion.
    const finalInst = await f.instance(processInstanceId)
    assert.equal(finalInst!.status, 'completed')
    assert.equal(finalInst!.outcome, 'completed')
    assert.equal((await f.events(processInstanceId, 'task.completed')).length, 2)
    assert.equal(await taskAdvanceCount(f, processInstanceId, ['tA', 'tB']), 2)
    assert.equal((await f.events(processInstanceId, 'token.joined')).length, 1)
    assert.equal((await f.events(processInstanceId, 'process.completed')).length, 1)
    await assertNoActionableTasks(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

// ---------------------------------------------------------------------------
// duplicate-completion safety — exactly-once completion and token advancement
// ---------------------------------------------------------------------------

test('ENG-13: sequential duplicate completion is a deterministic conflict — no double advance', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const engine = f.makeEngine()
    const { processInstanceId } = await startLinear(f, engine)
    const taskId = ((await f.tasks(processInstanceId)) as Array<{ id: string }>)[0].id

    await engine.completeTask({ taskId, userId: 'u1', transitionName: 'done', formData: { ok: true } })

    // The second completion is rejected before any mutation or event.
    await assert.rejects(
      () => engine.completeTask({ taskId, userId: 'u1', transitionName: 'done' }),
      (e: any) =>
        e.code === 'TASK_ALREADY_COMPLETED' &&
        String(e.message).startsWith('Task cannot be completed in status: completed'),
    )

    await assertExactlyOnceCompletion(f, processInstanceId)
    const tasks = await f.tasks(processInstanceId)
    assert.deepEqual(tasks[0].form_data, { ok: true }, 'first completion form data retained')
  } finally {
    await f.cleanup()
  }
})

test('ENG-13: concurrent duplicate completions — exactly one winner, exactly-once token advance', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const engine = f.makeEngine()

    for (let i = 0; i < 6; i++) {
      const { processInstanceId } = await startLinear(f, engine)
      const taskId = ((await f.tasks(processInstanceId)) as Array<{ id: string }>)[0].id

      // Two users race to complete the same ready task. Exactly one wins; the
      // loser must be rejected deterministically — never a double completion.
      const results = await Promise.allSettled([
        engine.completeTask({ taskId, userId: 'u1', transitionName: 'done' }),
        engine.completeTask({ taskId, userId: 'u2', transitionName: 'done' }),
      ])
      const fulfilled = results.filter((r) => r.status === 'fulfilled').length
      assert.equal(fulfilled, 1, `iteration ${i}: exactly one completion wins`)
      const rejected = results.filter((r) => r.status === 'rejected')
      assert.equal(rejected.length, 1, `iteration ${i}: exactly one completion loses`)
      assert.equal(
        (rejected[0] as PromiseRejectedResult).reason?.code,
        'TASK_ALREADY_COMPLETED',
        `iteration ${i}: loser sees TASK_ALREADY_COMPLETED`,
      )

      await assertExactlyOnceCompletion(f, processInstanceId)
    }
  } finally {
    await f.cleanup()
  }
})

// ---------------------------------------------------------------------------
// claim-vs-complete race
// ---------------------------------------------------------------------------

test('ENG-13: claim-vs-complete — deterministic interleaving: claim wins, completion is rejected', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const gate = new HookGate()
    const engine = f.makeEngine({
      hooks: {
        // claimTask fires this hook after its task UPDATE while still holding
        // the instance + task row locks — a real overlapping transaction.
        beforeTaskCompleteEvent: async () => {
          await gate.hold()
        },
      },
    })
    const { processInstanceId } = await startLinear(f, engine)
    const taskId = ((await f.tasks(processInstanceId)) as Array<{ id: string }>)[0].id

    // The claim parks holding its locks; the completion blocks on the instance
    // row (ENG-11 lock ordering) — a real lock wait, not a fake.
    const claiming = engine.claimTask(taskId, 'u1')
    await gate.entered

    const completing = engine.completeTask({ taskId, userId: 'u2', transitionName: 'done' })
    gate.open()

    const [claimResult, completeResult] = await Promise.allSettled([claiming, completing])
    assert.equal(claimResult.status, 'fulfilled', 'claim settles cleanly (no deadlock)')
    assert.equal(completeResult.status, 'rejected', 'completion must lose to the claim')
    assert.equal(
      (completeResult as PromiseRejectedResult).reason?.code,
      'TASK_ASSIGNEE_ONLY',
      'u2 cannot complete a task claimed by u1',
    )

    const tasks = await f.tasks(processInstanceId)
    assert.equal(tasks[0].status, 'reserved')
    assert.equal(tasks[0].assignee, 'u1')
    assert.equal((await f.events(processInstanceId, 'task.completed')).length, 0)
    assert.equal(await taskAdvanceCount(f, processInstanceId, ['review']), 0)

    const inst = await f.instance(processInstanceId)
    assert.equal(inst!.status, 'active')
    const tokens = await f.tokens(processInstanceId)
    assert.equal(tokens[0].node_id, 'review', 'token never advanced')

    // The claim holder can still complete exactly once.
    await engine.completeTask({ taskId, userId: 'u1', transitionName: 'done' })
    await assertExactlyOnceCompletion(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-13: claim-vs-complete — concurrent stress (either order, invariant holds)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const engine = f.makeEngine()

    for (let i = 0; i < 6; i++) {
      const { processInstanceId } = await startLinear(f, engine)
      const taskId = ((await f.tasks(processInstanceId)) as Array<{ id: string }>)[0].id

      const results = await Promise.allSettled([
        engine.claimTask(taskId, 'u1'),
        engine.completeTask({ taskId, userId: 'u2', transitionName: 'done' }),
      ])
      assert.ok(
        results.every(isDeterministicTaskRejection),
        `iteration ${i}: unexpected rejection ${JSON.stringify(
          results.filter((r) => !isDeterministicTaskRejection(r)).map((r: any) => r.reason?.message ?? r.reason),
        )}`,
      )

      const inst = await f.instance(processInstanceId)
      assert.ok(inst, 'instance row must exist')
      const completedEvents = await f.events(processInstanceId, 'task.completed')
      assert.ok(
        completedEvents.length <= 1,
        `iteration ${i}: at most one completion (got ${completedEvents.length})`,
      )
      const advances = await taskAdvanceCount(f, processInstanceId, ['review'])
      assert.ok(
        advances <= 1,
        `iteration ${i}: at most one token advance past the task (got ${advances})`,
      )

      if (inst.status === 'completed') {
        // Completion won: exactly-once completion proof.
        assert.equal(completedEvents.length, 1, `iteration ${i}: completion recorded once`)
        assert.equal(advances, 1, `iteration ${i}: token advanced once`)
      } else {
        // Claim won: the task is reserved by u1, nothing completed, nothing
        // advanced — the claim holder may still complete exactly once.
        assert.equal(inst.status, 'active')
        assert.equal(completedEvents.length, 0)
        assert.equal(advances, 0)
        const tasks = (await f.tasks(processInstanceId)) as Array<{
          id: string
          status: string
          assignee: string | null
        }>
        assert.equal(tasks[0].status, 'reserved')
        assert.equal(tasks[0].assignee, 'u1')
        await engine.completeTask({ taskId: tasks[0].id, userId: 'u1', transitionName: 'done' })
        await assertExactlyOnceCompletion(f, processInstanceId)
      }
    }
  } finally {
    await f.cleanup()
  }
})

// ---------------------------------------------------------------------------
// completion-vs-cancellation race
// ---------------------------------------------------------------------------

test('ENG-13: completion-vs-cancellation — deterministic: cancellation wins, completion is rejected', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const gate = new HookGate()
    const engine = f.makeEngine({
      hooks: {
        beforeProcessTerminal: async () => {
          await gate.hold()
        },
      },
    })
    const { processInstanceId } = await startLinear(f, engine)
    const taskId = ((await f.tasks(processInstanceId)) as Array<{ id: string }>)[0].id

    // Cancellation parks mid-terminal-transition holding the instance lock;
    // the human completion blocks on it.
    const cancelling = engine.cancelProcess({ processInstanceId, actor: 'boss' })
    await gate.entered

    const completing = engine.completeTask({ taskId, userId: 'u1', transitionName: 'done' })
    gate.open()

    const [cancelResult, completeResult] = await Promise.allSettled([cancelling, completing])
    assert.equal(cancelResult.status, 'fulfilled')
    assert.equal(completeResult.status, 'rejected', 'completion must fail on a cancelled process')
    // The completion re-reads the obsoleted task and is rejected on its
    // status before the process-level check — a deterministic conflict.
    assert.equal(
      (completeResult as PromiseRejectedResult).reason?.code,
      'TASK_NOT_ACTIONABLE',
    )

    const inst = await f.instance(processInstanceId)
    assert.equal(inst!.status, 'aborted')
    assert.equal(inst!.outcome, 'cancelled')
    assert.equal((await f.events(processInstanceId, 'process.cancelled')).length, 1)
    assert.equal((await f.events(processInstanceId, 'task.completed')).length, 0)
    assert.equal(await taskAdvanceCount(f, processInstanceId, ['review']), 0)
    await assertNoActionableTasks(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-13: completion-vs-cancellation — deterministic: advance wins, cancellation is rejected', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const gate = new HookGate()
    const engine = f.makeEngine({
      hooks: {
        // The completion's token has already moved to the end node and the
        // task is completed, but the process is not yet marked completed —
        // the instance lock is held by the completion.
        beforeNodeArrive: async (nodeId: string) => {
          if (nodeId === 'end') await gate.hold()
        },
      },
    })
    const { processInstanceId } = await startLinear(f, engine)
    const taskId = ((await f.tasks(processInstanceId)) as Array<{ id: string }>)[0].id

    const completing = engine.completeTask({ taskId, userId: 'u1', transitionName: 'done' })
    await gate.entered

    const cancelling = engine.cancelProcess({ processInstanceId, actor: 'boss' })
    gate.open()

    const [completeResult, cancelResult] = await Promise.allSettled([completing, cancelling])
    assert.equal(completeResult.status, 'fulfilled', 'completion settles cleanly (no deadlock)')
    assert.equal(cancelResult.status, 'rejected', 'cancellation must fail on a completed process')
    assert.equal((cancelResult as PromiseRejectedResult).reason?.code, 'PROCESS_NOT_ACTIVE')

    await assertExactlyOnceCompletion(f, processInstanceId)
    assert.equal((await f.events(processInstanceId, 'process.cancelled')).length, 0)
    await assertNoActionableTasks(f, processInstanceId)
  } finally {
    await f.cleanup()
  }
})

test('ENG-13: completion-vs-cancellation — concurrent stress (either order, no resurrection, no residue)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const engine = f.makeEngine()

    for (let i = 0; i < 6; i++) {
      const { processInstanceId } = await startLinear(f, engine)
      const taskId = ((await f.tasks(processInstanceId)) as Array<{ id: string }>)[0].id

      const results = await Promise.allSettled([
        engine.completeTask({ taskId, userId: 'u1', transitionName: 'done' }),
        engine.cancelProcess({ processInstanceId, actor: 'boss' }),
      ])
      assert.ok(
        results.every(isDeterministicTaskRejection),
        `iteration ${i}: unexpected rejection ${JSON.stringify(
          results.filter((r) => !isDeterministicTaskRejection(r)).map((r: any) => r.reason?.message ?? r.reason),
        )}`,
      )

      const inst = await f.instance(processInstanceId)
      assert.ok(
        inst!.status === 'aborted' || inst!.status === 'completed',
        `iteration ${i}: instance must be terminal (status=${inst!.status})`,
      )
      // Exactly one terminal event, never both.
      const cancelled = await f.events(processInstanceId, 'process.cancelled')
      const completed = await f.events(processInstanceId, 'process.completed')
      assert.equal(
        cancelled.length + completed.length,
        1,
        `iteration ${i}: exactly one terminal event`,
      )
      assert.ok(
        (await f.events(processInstanceId, 'task.completed')).length <= 1,
        `iteration ${i}: at most one task.completed`,
      )
      assert.ok(
        (await taskAdvanceCount(f, processInstanceId, ['review'])) <= 1,
        `iteration ${i}: at most one token advance past the task`,
      )
      await assertNoActionableTasks(f, processInstanceId)
    }
  } finally {
    await f.cleanup()
  }
})

// ---------------------------------------------------------------------------
// claim-vs-cancellation race
// ---------------------------------------------------------------------------

test('ENG-13: claim-vs-cancellation — a claimed task is obsoleted by termination (no residue)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const gate = new HookGate()
    const engine = f.makeEngine({
      hooks: {
        beforeTaskCompleteEvent: async () => {
          await gate.hold()
        },
      },
    })
    const { processInstanceId } = await startLinear(f, engine)
    const taskId = ((await f.tasks(processInstanceId)) as Array<{ id: string }>)[0].id

    // The claim parks holding instance + task locks; cancellation blocks on
    // the instance row, then obsoletes the freshly claimed task.
    const claiming = engine.claimTask(taskId, 'u1')
    await gate.entered

    const cancelling = engine.cancelProcess({ processInstanceId, actor: 'boss' })
    gate.open()

    const [claimResult, cancelResult] = await Promise.allSettled([claiming, cancelling])
    assert.equal(claimResult.status, 'fulfilled', 'claim settles cleanly (no deadlock)')
    assert.equal(cancelResult.status, 'fulfilled', 'cancellation settles cleanly (no deadlock)')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst!.status, 'aborted')
    assert.equal(inst!.outcome, 'cancelled')
    const tasks = (await f.tasks(processInstanceId)) as Array<{ status: string }>
    assert.equal(tasks[0].status, 'obsolete', 'termination obsoletes the claimed task')
    assert.equal((await f.events(processInstanceId, 'task.claimed')).length, 1)
    assert.equal((await f.events(processInstanceId, 'task.completed')).length, 0)
    assert.equal((await f.events(processInstanceId, 'process.cancelled')).length, 1)
    await assertNoActionableTasks(f, processInstanceId)

    // The claimed-then-obsoleted task is not actionable and not surfaced.
    const active = await engine.getActiveTasksForUser('u1', f.tenantId)
    assert.equal(
      active.some((t: any) => t.processInstanceId === processInstanceId),
      false,
      'obsoleted task must not surface in the active task list',
    )
  } finally {
    await f.cleanup()
  }
})

// ---------------------------------------------------------------------------
// terminal cleanup — no actionable tasks after terminal process state
// ---------------------------------------------------------------------------

test('ENG-13: after a terminal process state every task operation is a deterministic conflict', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_ht', 1, LINEAR_TASK)
    const engine = f.makeEngine()

    // --- Cancelled process ---
    const cancelledId = (await startLinear(f, engine)).processInstanceId
    const cancelledTaskId = ((await f.tasks(cancelledId)) as Array<{ id: string }>)[0].id
    await engine.cancelProcess({ processInstanceId: cancelledId, actor: 'boss' })
    await assert.rejects(
      () => engine.claimTask(cancelledTaskId, 'u1'),
      (e: any) => e.code === 'PROCESS_NOT_ACTIVE',
    )
    await assert.rejects(
      () => engine.releaseTask(cancelledTaskId, 'u1'),
      (e: any) => e.code === 'PROCESS_NOT_ACTIVE',
    )
    await assert.rejects(
      () => engine.reassignTask(cancelledTaskId, 'u2', 'manager'),
      (e: any) => e.code === 'PROCESS_NOT_ACTIVE',
    )
    await assert.rejects(
      () => engine.completeTask({ taskId: cancelledTaskId, userId: 'u1', transitionName: 'done' }),
      (e: any) => e.code === 'TASK_NOT_ACTIONABLE',
    )
    await assertNoActionableTasks(f, cancelledId)
    assert.equal(
      (await f.events(cancelledId, 'task.obsoleted')).length,
      1,
      'the open task was obsoleted exactly once',
    )

    // --- Completed process ---
    const completedId = (await startLinear(f, engine)).processInstanceId
    const completedTaskId = ((await f.tasks(completedId)) as Array<{ id: string }>)[0].id
    await engine.completeTask({ taskId: completedTaskId, userId: 'u1', transitionName: 'done' })
    await assert.rejects(
      () => engine.claimTask(completedTaskId, 'u1'),
      (e: any) => e.code === 'PROCESS_NOT_ACTIVE',
    )
    await assert.rejects(
      () => engine.releaseTask(completedTaskId, 'u1'),
      (e: any) => e.code === 'PROCESS_NOT_ACTIVE',
    )
    await assert.rejects(
      () => engine.reassignTask(completedTaskId, 'u2', 'manager'),
      (e: any) => e.code === 'PROCESS_NOT_ACTIVE',
    )
    await assert.rejects(
      () => engine.completeTask({ taskId: completedTaskId, userId: 'u1', transitionName: 'done' }),
      (e: any) => e.code === 'TASK_ALREADY_COMPLETED',
    )
    await assertNoActionableTasks(f, completedId)

    // Neither terminal instance surfaces any active task for its users.
    for (const id of [cancelledId, completedId]) {
      const active = await engine.getActiveTasksForUser('u1', f.tenantId)
      assert.equal(
        active.some((t: any) => t.processInstanceId === id),
        false,
        `no actionable task surfaces for terminal instance ${id}`,
      )
    }
  } finally {
    await f.cleanup()
  }
})
