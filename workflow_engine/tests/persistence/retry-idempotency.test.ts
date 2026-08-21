import { test } from 'node:test'
import assert from 'node:assert/strict'

import { withTransaction } from '../../../lib/neon-interactive'
import {
  claimReceipt,
  finalizeReceipt,
  readFinalReceipt,
  replayOutcome,
} from '../../../db/workflow-command-receipt'
import { assertEngineSchema, ensureCommandEffectTable, PersistenceFixture } from './harness'

// start -> approve (human task) -> do (application command) -> end
const COMMAND_LINE = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'approve' }] },
    approve: {
      id: 'approve',
      type: 'task',
      name: 'Approve',
      transitions: [{ name: 'done', to: 'do' }],
    },
    do: {
      id: 'do',
      type: 'command',
      commandType: 'tunit.do',
      transitions: [{ name: 'go', to: 'end' }],
    },
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

function makeIdempotentApp(f: PersistenceFixture, executed: string[]) {
  return {
    async executeCommand(req: { commandId: string }) {
      executed.push(req.commandId)
      return withTransaction(async (tx) => {
        const claimed = await claimReceipt(tx, req.commandId)
        if (claimed) {
          await tx`insert into tunit_command_effect (command_id, tenant_id, effect_count) values (${req.commandId}, ${f.tenantId}, 1)`
          await finalizeReceipt(tx, req.commandId, 'success', null, null)
          return { commandId: req.commandId, outcome: 'success' as const, message: null }
        }
        const receipt = await readFinalReceipt(tx, req.commandId)
        const replay = replayOutcome(receipt)
        return { commandId: req.commandId, outcome: replay.outcome, message: replay.message }
      })
    },
    async readFacts() {
      return {}
    },
  }
}

async function startTimer(f: PersistenceFixture, engine: any) {
  return engine.startProcess({
    definitionKey: 'tunit_retry',
    version: 1,
    tenantId: f.tenantId,
    startedBy: 'tester',
  })
}

test('ENG-10: a retryable job fails then succeeds — attempts persist, no duplicate fire', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_retry', 1, TIMER_END)
    let attempts = 0
    let nowMs = T0.getTime() + 10_000
    const engine = f.makeEngine({
      now: () => new Date(nowMs),
      hooks: {
        beforeTimerTokenMove: async () => {
          attempts += 1
          if (attempts === 1) throw new Error('transient failure on first fire')
        },
      },
    })
    const { processInstanceId } = await startTimer(f, engine)

    // First attempt: the timer job is claimed, execution fails transiently.
    const first = await engine.runDueJobs('w', 10)
    assert.equal(first.failed, 1, 'first fire failed transiently')

    const job = (await f.jobs(processInstanceId))[0]
    assert.equal(job.status, 'pending', 'retryable failure returns to pending')
    assert.ok(job.attempts >= 1)

    // Advance the clock past the backoff so the retry is due again.
    nowMs += 120_000

    // Second attempt succeeds.
    const second = await engine.runDueJobs('w', 10)
    assert.equal(second.fired, 1, 'retry fires exactly once')
    const tokens = await f.tokens(processInstanceId)
    assert.equal(tokens.filter((t: any) => t.node_id === 'end').length, 1)
    const evs = await f.events(processInstanceId, 'timer.fired')
    assert.equal(evs.length, 1, 'one timer.fired event')
  } finally {
    await f.cleanup()
  }
})

test('ENG-10: replay after a committed side effect returns the same command outcome (no duplicate business effect)', async () => {
  await assertEngineSchema()
  await ensureCommandEffectTable()
  const f = new PersistenceFixture()
  const executed: string[] = []
  try {
    await f.seedDefinition('tunit_retry', 1, COMMAND_LINE)
    let armed = true
    const engine = f.makeEngine({
      app: makeIdempotentApp(f, executed),
      hooks: {
        afterCommandSideEffect: async () => {
          if (armed) throw new Error('engine crash after side effect')
        },
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_retry',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }

    // Crash after the side effect committed but before engine commit.
    await assert.rejects(() =>
      engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' }),
    )
    let effects = await f.rows<{ c: number }>`select count(*)::int as c from tunit_command_effect where tenant_id = ${f.tenantId}`
    assert.equal(effects[0].c, 1, 'side effect committed once')

    // Replay: same deterministic commandId, stored outcome, no new effect.
    armed = false
    await engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' })
    effects = await f.rows<{ c: number }>`select count(*)::int as c from tunit_command_effect where tenant_id = ${f.tenantId}`
    assert.equal(effects[0].c, 1, 'side effect NOT duplicated on replay')
    assert.equal(executed[0], executed[1], 'deterministic commandId reused')
  } finally {
    await f.cleanup()
  }
})

test('ENG-10: duplicate command delivery creates one business side effect (idempotent replay)', async () => {
  await assertEngineSchema()
  await ensureCommandEffectTable()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_retry', 1, COMMAND_LINE)
    const engine = f.makeEngine({ app: makeIdempotentApp(f, []) })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_retry',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }

    await engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' })
    const effects = await f.rows<{ c: number }>`select count(*)::int as c from tunit_command_effect where tenant_id = ${f.tenantId}`
    assert.equal(effects[0].c, 1, 'exactly one business side effect')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')
  } finally {
    await f.cleanup()
  }
})

test('ENG-10: permanent business rejection is terminal, never retried as infrastructure', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_retry', 1, COMMAND_LINE)
    const engine = f.makeEngine({
      app: {
        async executeCommand() {
          return { commandId: 'x', outcome: 'validation_failure' as const, message: 'bad input' }
        },
        async readFacts() {
          return {}
        },
      },
    })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_retry',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }

    await engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' })

    // The process terminates 'failed' immediately — no retry loop.
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'error')
    assert.equal(inst.outcome, 'failed')
    const cmds = await f.commands(processInstanceId)
    assert.equal(cmds.length, 1)
    assert.equal(cmds[0].outcome, 'validation_failure')
  } finally {
    await f.cleanup()
  }
})

test('ENG-10: explicit operator retry of an exhausted job is deterministic and audited', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_retry', 1, TIMER_END)
    let nowMs = T0.getTime() + 10_000
    const engine = f.makeEngine({ now: () => new Date(nowMs) })
    const { processInstanceId } = await startTimer(f, engine)
    const jobId = (await f.jobs(processInstanceId))[0].id

    for (let i = 0; i < 5; i++) {
      const claimed = await engine.claimJobs('w', 1)
      if (claimed.length !== 1) throw new Error(`iteration ${i}: expected 1 claim`)
      await engine.failJob(jobId, 'w', `boom ${i}`)
      nowMs += 30 * 60_000 // advance well past backoff so it becomes due again
    }
    let job = await engine.getJob(jobId)
    assert.equal(job!.status, 'failed')

    // Operator requeue resets deterministically; the job can run again.
    await engine.requeueJob({ jobId, actor: 'operator' })
    job = await engine.getJob(jobId)
    assert.equal(job!.status, 'pending')
    assert.equal(job!.attempts, 0)

    const res = await engine.runDueJobs('w', 10)
    assert.equal(res.fired, 1, 'requeued job runs to completion')
    const evs = await f.events(processInstanceId, 'job.requeued')
    assert.equal(evs.length, 1)
  } finally {
    await f.cleanup()
  }
})
