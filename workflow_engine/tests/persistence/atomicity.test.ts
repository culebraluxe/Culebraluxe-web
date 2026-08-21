import { test } from 'node:test'
import assert from 'node:assert/strict'

import { assertEngineSchema, PersistenceFixture } from './harness'

// start -> tA (task) -> end
const LINEAR_TASK = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'tA' }] },
    tA: { id: 'tA', type: 'task', name: 'A', transitions: [{ name: 'done', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

// start -> fork(tA, tB required) -> join -> end
const FORK2 = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'fork' }] },
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

test('ENG-09: fault mid-task-completion rolls back token, events, and task (no partial persistent state)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_at_a', 1, LINEAR_TASK)
    const engine = f.makeEngine({
      hooks: {
        beforeNodeArrive: async (nodeId) => {
          if (nodeId === 'end') throw new Error('injected failure at end arrival')
        },
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_at_a',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }

    await assert.rejects(() =>
      engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' }),
    )

    const tasks = await f.tasks(processInstanceId)
    assert.equal(tasks[0].status, 'ready', 'task completion rolled back')
    const tokens = await f.tokens(processInstanceId)
    assert.equal(tokens.length, 1, 'no successor token')
    assert.equal(tokens[0].node_id, 'tA')
    assert.equal(tokens[0].status, 'active')
    const events = await f.events(processInstanceId)
    // A token.moved from startProcess is expected; the completing step's
    // events (task.completed / end-arrival move) must all be gone.
    assert.equal(events.some((e: any) => e.event_type === 'task.completed'), false)
    assert.equal(events.some((e: any) => e.event_type === 'process.completed'), false)
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'active')
  } finally {
    await f.cleanup()
  }
})

test('ENG-09: fault during fork child creation rolls back the entire start (no partial instances/tokens/events)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_at_b', 1, FORK2)
    const engine = f.makeEngine({
      hooks: {
        beforeForkChildCreate: async () => {
          throw new Error('injected fork child failure')
        },
      },
    })

    await assert.rejects(() =>
      engine.startProcess({
        definitionKey: 'tunit_at_b',
        version: 1,
        tenantId: f.tenantId,
        startedBy: 'tester',
      }),
    )

    const insts = await f.rows<{ c: number }>`select count(*)::int as c from process_instances where tenant_id = ${f.tenantId}`
    assert.equal(insts[0].c, 0, 'no process_instances row survives the rollback')
    const toks = await f.rows<{ c: number }>`select count(*)::int as c from tokens where tenant_id = ${f.tenantId}`
    assert.equal(toks[0].c, 0, 'no tokens survive the rollback')
    const evs = await f.rows<{ c: number }>`select count(*)::int as c from process_events where tenant_id = ${f.tenantId}`
    assert.equal(evs[0].c, 0, 'no events survive the rollback')
  } finally {
    await f.cleanup()
  }
})

test('ENG-09: fault after the join release rolls back the completing step; retry yields exactly one release', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_at_c', 1, FORK2)
    let armed = true
    const engine = f.makeEngine({
      hooks: {
        beforeNodeArrive: async (nodeId) => {
          if (armed && nodeId === 'end') throw new Error('injected failure at join successor')
        },
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_at_c',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const tasks = await f.tasks(processInstanceId)

    await engine.completeTask({ taskId: tasks[0].id, userId: 'u', transitionName: 'done' })
    await assert.rejects(() =>
      engine.completeTask({ taskId: tasks[1].id, userId: 'u', transitionName: 'done' }),
    )

    // No partial release state.
    let joined = await f.events(processInstanceId, 'token.joined')
    assert.equal(joined.length, 0, 'no join release while the completing step rolled back')
    let tokens = await f.tokens(processInstanceId)
    assert.equal(tokens.filter((t: any) => t.node_id === 'end').length, 0)

    // The rolled-back branch is re-open; retry it and assert exactly-once.
    const tasks2 = (await f.tasks(processInstanceId)) as Array<{ id: string; status: string }>
    const open = tasks2.filter((t: any) => t.status !== 'completed')
    assert.equal(open.length, 1)
    armed = false
    await engine.completeTask({ taskId: open[0].id, userId: 'u', transitionName: 'done' })

    joined = await f.events(processInstanceId, 'token.joined')
    assert.equal(joined.length, 1, 'exactly one join release after retry')
    tokens = await f.tokens(processInstanceId)
    assert.equal(tokens.filter((t: any) => t.node_id === 'end').length, 1)
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')
  } finally {
    await f.cleanup()
  }
})

test('ENG-09: fault during timer advancement rolls back job + token (no partial timer step)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_at_d', 1, TIMER_END)
    let armed = true
    const engine = f.makeEngine({
      now: () => new Date('2026-08-21T00:00:00.000Z'),
      hooks: {
        beforeTimerTokenMove: async () => {
          if (armed) throw new Error('injected timer failure')
        },
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_at_d',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })

    const jobs = await f.jobs(processInstanceId)
    assert.equal(jobs.length, 1)
    assert.equal(jobs[0].status, 'pending')
    const locked = await engine.claimJobs('w1', 1)
    assert.equal(locked.length, 1)

    await assert.rejects(() => engine.fireTimerJob({ jobId: jobs[0].id, workerId: 'w1' }))

    const jobs2 = await f.jobs(processInstanceId)
    assert.equal(jobs2[0].status, 'locked', 'job remains locked after the rollback')
    const tokens = await f.tokens(processInstanceId)
    assert.equal(tokens[0].node_id, 'timer')
    assert.equal(tokens[0].status, 'active')

    armed = false
    await engine.fireTimerJob({ jobId: jobs[0].id, workerId: 'w1' })
    const jobs3 = await f.jobs(processInstanceId)
    assert.equal(jobs3[0].status, 'completed')
    const tokens3 = await f.tokens(processInstanceId)
    assert.equal(tokens3.filter((t: any) => t.node_id === 'end').length, 1)
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')
  } finally {
    await f.cleanup()
  }
})

test('ENG-09: fault during the process terminal transition rolls back completion (process stays active)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_at_e', 1, LINEAR_TASK)
    let armed = true
    const engine = f.makeEngine({
      hooks: {
        beforeProcessTerminal: async () => {
          if (armed) throw new Error('injected terminal failure')
        },
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_at_e',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }

    await assert.rejects(() =>
      engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' }),
    )

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'active', 'process NOT completed after the rollback')
    const tasks = await f.tasks(processInstanceId)
    assert.equal(tasks[0].status, 'ready', 'task re-opened')
    const events = await f.events(processInstanceId)
    assert.equal(events.some((e: any) => e.event_type === 'process.completed'), false)
    assert.equal(events.some((e: any) => e.event_type === 'token.completed'), false)
  } finally {
    await f.cleanup()
  }
})

