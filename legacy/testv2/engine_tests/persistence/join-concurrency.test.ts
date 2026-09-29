import { test } from 'node:test'
import assert from 'node:assert/strict'

import { assertEngineSchema, PersistenceFixture } from './harness'

// start -> fork(tA, tB required) -> join -> end
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

// start -> fork(10 required tasks) -> join -> end
const FORK10 = {
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
      transitions: Array.from({ length: 10 }, (_, i) => ({
        name: 'b' + i,
        to: 'w' + i,
        required: true,
      })),
    },
    ...Object.fromEntries(
      Array.from({ length: 10 }, (_, i) => [
        'w' + i,
        {
          id: 'w' + i,
          type: 'task',
          name: 'W' + i,
          transitions: [{ name: 'done', to: 'join' }],
        },
      ]),
    ),
    join: { id: 'join', type: 'join', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

async function branchTasks(
  f: PersistenceFixture,
  instanceId: string,
): Promise<Array<{ id: string; name: string }>> {
  const tasks = await f.tasks(instanceId)
  return tasks.map((t: any) => ({ id: t.id, name: t.name }))
}

test('CRM-14B: two required branches completing concurrently release the join exactly once (real Postgres)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_fj', 1, FORK_JOIN)
    const engine = f.makeEngine()

    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_fj',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })

    const tasks = await branchTasks(f, processInstanceId)
    assert.equal(tasks.length, 2)

    await Promise.all([
      engine.completeTask({ taskId: tasks[0].id, userId: 'u', transitionName: 'done' }),
      engine.completeTask({ taskId: tasks[1].id, userId: 'u', transitionName: 'done' }),
    ])

    const joined = await f.events(processInstanceId, 'token.joined')
    assert.equal(joined.length, 1, 'exactly one token.joined event')

    const tokens = await f.tokens(processInstanceId)
    const downstream = tokens.filter((t: any) => t.node_id === 'end')
    assert.equal(downstream.length, 1, 'exactly one downstream successor token')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')
    assert.equal(inst.outcome, 'completed')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14B: exactly-once join release holds over repeated concurrent completions', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_fj', 1, FORK_JOIN)
    const engine = f.makeEngine()

    for (let i = 0; i < 8; i++) {
      const { processInstanceId } = await engine.startProcess({
        definitionKey: 'tunit_fj',
        version: 1,
        tenantId: f.tenantId,
        startedBy: 'tester',
      })
      const tasks = await branchTasks(f, processInstanceId)
      await Promise.all([
        engine.completeTask({ taskId: tasks[0].id, userId: 'u', transitionName: 'done' }),
        engine.completeTask({ taskId: tasks[1].id, userId: 'u', transitionName: 'done' }),
      ])
      const joined = await f.events(processInstanceId, 'token.joined')
      assert.equal(joined.length, 1, 'iteration ' + i + ': exactly one token.joined event')
      const tokens = await f.tokens(processInstanceId)
      const downstream = tokens.filter((t: any) => t.node_id === 'end')
      assert.equal(downstream.length, 1, 'iteration ' + i + ': exactly one downstream token')
    }
  } finally {
    await f.cleanup()
  }
})

test('CRM-14B: rollback of one join contender leaves no corruption; retry releases exactly once', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_fj', 1, FORK_JOIN)

    let injected = false
    const hooks = {
      afterJoinParentLock: async () => {
        if (!injected) {
          injected = true
          throw new Error('injected join-contender rollback')
        }
      },
    }
    const engine = f.makeEngine({ hooks })

    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_fj',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const tasks = await branchTasks(f, processInstanceId)

    const results = await Promise.allSettled([
      engine.completeTask({ taskId: tasks[0].id, userId: 'u', transitionName: 'done' }),
      engine.completeTask({ taskId: tasks[1].id, userId: 'u', transitionName: 'done' }),
    ])
    assert.equal(
      results.some((r) => r.status === 'rejected'),
      true,
      'one contender rolled back',
    )

    // No premature release and no partial successor state.
    let joined = await f.events(processInstanceId, 'token.joined')
    assert.equal(joined.length, 0, 'no join release while a contender rolled back')
    const tokens0 = await f.tokens(processInstanceId)
    assert.equal(tokens0.filter((t: any) => t.node_id === 'end').length, 0)

    // The rolled-back branch task is re-open; retry it.
    const tasks2 = (await f.tasks(processInstanceId)) as Array<{ id: string; status: string }>
    const openTask = tasks2.find((t: any) => t.status !== 'completed')
    assert.ok(openTask, 'the rolled-back branch task is re-open after rollback')
    await engine.completeTask({ taskId: openTask.id, userId: 'u', transitionName: 'done' })

    joined = await f.events(processInstanceId, 'token.joined')
    assert.equal(joined.length, 1, 'exactly one join release after retry')
    const tokens = await f.tokens(processInstanceId)
    assert.equal(
      tokens.filter((t: any) => t.node_id === 'end').length,
      1,
    )
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14B: unrelated process instances are not unnecessarily serialized', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_fj', 1, FORK_JOIN)
    const engine = f.makeEngine()

    const a = await engine.startProcess({
      definitionKey: 'tunit_fj',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const b = await engine.startProcess({
      definitionKey: 'tunit_fj',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })

    const [aTasks, bTasks] = await Promise.all([
      branchTasks(f, a.processInstanceId),
      branchTasks(f, b.processInstanceId),
    ])

    await Promise.all([
      engine.completeTask({ taskId: aTasks[0].id, userId: 'u', transitionName: 'done' }),
      engine.completeTask({ taskId: aTasks[1].id, userId: 'u', transitionName: 'done' }),
      engine.completeTask({ taskId: bTasks[0].id, userId: 'u', transitionName: 'done' }),
      engine.completeTask({ taskId: bTasks[1].id, userId: 'u', transitionName: 'done' }),
    ])

    for (const id of [a.processInstanceId, b.processInstanceId]) {
      const joined = await f.events(id, 'token.joined')
      assert.equal(joined.length, 1, 'instance ' + id + ': exactly one release')
      const inst = await f.instance(id)
      assert.equal(inst.status, 'completed')
    }
  } finally {
    await f.cleanup()
  }
})

test('CRM-14B: a 10-way required fork releases exactly once under concurrent completion (residential shape)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_fork10', 1, FORK10)
    const engine = f.makeEngine()

    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_fork10',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })

    const tasks = await branchTasks(f, processInstanceId)
    assert.equal(tasks.length, 10)

    await Promise.all(
      tasks.map((t) =>
        engine.completeTask({ taskId: t.id, userId: 'u', transitionName: 'done' }),
      ),
    )

    const joined = await f.events(processInstanceId, 'token.joined')
    assert.equal(joined.length, 1, 'exactly one token.joined event for 10-way fork')
    const tokens = await f.tokens(processInstanceId)
    assert.equal(
      tokens.filter((t: any) => t.node_id === 'end').length,
      1,
    )
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')
  } finally {
    await f.cleanup()
  }
})

