import { test } from 'node:test'
import assert from 'node:assert/strict'

import { assertEngineSchema, PersistenceFixture } from './harness'

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

// start -> timer -> end (used for claiming only; we assert lease columns)
const T0 = new Date('2026-08-21T00:00:00.000Z')

async function startTimerInstance(f: PersistenceFixture, engine: any) {
  return engine.startProcess({
    definitionKey: 'tunit_lease',
    version: 1,
    tenantId: f.tenantId,
    startedBy: 'tester',
  })
}

test('CRM-14F: fresh due job is claimed exactly once by one worker (real Postgres)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })
    const { processInstanceId } = await startTimerInstance(f, engine)

    const jobs = await f.jobs(processInstanceId)
    assert.equal(jobs.length, 1)
    assert.equal(jobs[0].status, 'pending')

    const claimed = await engine.claimJobs('worker-a', 1)
    assert.equal(claimed.length, 1)
    assert.equal(claimed[0].status, 'locked')
    assert.equal(claimed[0].lockedBy, 'worker-a')
    assert.ok((claimed[0].lockedUntil as Date).getTime() > T0.getTime())

    // A second claim attempt must NOT get the same job.
    const second = await engine.claimJobs('worker-b', 5)
    assert.equal(second.length, 0, 'locked job is not claimable by another worker')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14F: two workers racing for the same due job yield exactly one claim', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })
    const { processInstanceId } = await startTimerInstance(f, engine)

    const results = await Promise.all([
      engine.claimJobs('race-a', 5),
      engine.claimJobs('race-b', 5),
    ])
    const total = results.reduce((n, r) => n + r.length, 0)
    assert.equal(total, 1, 'exactly one worker claims the job under contention')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14F: stale locked job is reclaimed to pending after lease expiry; live lock is never reclaimed early', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    let nowMs = T0.getTime() + 10_000
    const engine = f.makeEngine({ now: () => new Date(nowMs) })
    const { processInstanceId } = await startTimerInstance(f, engine)

    const jobs = await f.jobs(processInstanceId)
    const jobId = jobs[0].id
    await engine.claimJobs('dead-worker', 1)

    // Lease is live: reclaim must NOT touch it.
    nowMs += 60_000 // within the 5-minute lease
    let reclaimed = await engine.reclaimStaleJobs(10)
    assert.equal(reclaimed, 0, 'live lock is never reclaimed early')

    // Lease expires: reclaim returns it to pending.
    nowMs += 5 * 60_000 + 1
    reclaimed = await engine.reclaimStaleJobs(10)
    assert.equal(reclaimed, 1, 'stale lock reclaimed exactly once')

    const after = await f.jobs(processInstanceId)
    assert.equal(after[0].status, 'pending')
    assert.equal(after[0].locked_by, null)
    assert.equal(after[0].locked_until, null)
    assert.ok(after[0].attempts >= 1, 'reclaim does not reset attempts')

    // Now a live worker can claim and complete it.
    const claimed = await engine.claimJobs('live-worker', 1)
    assert.equal(claimed.length, 1)
  } finally {
    await f.cleanup()
  }
})

test('CRM-14F: stale reclaim race yields exactly one reclaimer', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    let nowMs = T0.getTime() + 10_000
    const engine = f.makeEngine({ now: () => new Date(nowMs) })
    const { processInstanceId } = await startTimerInstance(f, engine)
    await engine.claimJobs('dead-worker', 1)
    nowMs += 6 * 60_000
    // Recreate the engine so both reclaimers see the same clock.
    const engine2 = f.makeEngine({ now: () => new Date(nowMs) })

    const results = await Promise.all([
      engine2.reclaimStaleJobs(10),
      engine2.reclaimStaleJobs(10),
    ])
    const total = results.reduce((n, r) => n + r, 0)
    assert.equal(total, 1, 'exactly one reclaimer wins the stale lease race')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14F: failJob increments attempts with deterministic backoff; permanent rejection is terminal immediately', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    let nowMs = T0.getTime() + 10_000
    const engine = f.makeEngine({ now: () => new Date(nowMs) })
    const { processInstanceId } = await startTimerInstance(f, engine)
    const jobId = (await f.jobs(processInstanceId))[0].id

    await engine.claimJobs('w', 1)
    await engine.failJob(jobId, 'w', 'transient infra error')
    let job = await engine.getJob(jobId)
    assert.equal(job!.status, 'pending', 'retryable failure returns to pending')
    assert.equal(job!.attempts, 1)
    assert.ok((job!.dueAt as Date).getTime() >= nowMs + 60_000, 'backoff pushes due_at forward')
    assert.match(job!.lastError ?? '', /transient/)

    // Advance past the backoff so the job is claimable again.
    nowMs += 120_000
    await engine.claimJobs('w', 1)
    await engine.failJob(jobId, 'w', 'business rejection', { permanent: true })
    job = await engine.getJob(jobId)
    assert.equal(job!.status, 'failed')
    assert.match(job!.lastError ?? '', /business rejection/)
  } finally {
    await f.cleanup()
  }
})


test('CRM-14F: max_attempts exhaustion lands the job terminal failed; no auto-resurrection; operator requeue is the only recovery', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    let nowMs = T0.getTime() + 10_000
    const engine = f.makeEngine({ now: () => new Date(nowMs) })
    const { processInstanceId } = await startTimerInstance(f, engine)
    const jobId = (await f.jobs(processInstanceId))[0].id

    // Exhaust the default max_attempts=5 via claim + fail cycles. Advance the
    // clock well past every backoff (max 60s*2^4=16min) so the job stays due.
    for (let i = 0; i < 5; i++) {
      const claimed = await engine.claimJobs('w', 1)
      if (claimed.length !== 1) throw new Error(`iteration ${i}: expected 1 claim`)
      await engine.failJob(jobId, 'w', `attempt ${i + 1} failed`)
      nowMs += 30 * 60_000 // past any backoff
    }
    let job = await engine.getJob(jobId)
    assert.equal(job!.status, 'failed')
    assert.equal(job!.attempts, 5)

    // Exhausted job is not reclaimed and not claimable.
    const reclaimed = await engine.reclaimStaleJobs(10)
    assert.equal(reclaimed, 0)
    const claimed = await engine.claimJobs('w2', 5)
    assert.equal(claimed.length, 0, 'failed job is never auto-claimed')

    // Explicit operator requeue is the only recovery path.
    await engine.requeueJob({ jobId, actor: 'operator' })
    job = await engine.getJob(jobId)
    assert.equal(job!.status, 'pending')
    assert.equal(job!.attempts, 0)
    assert.equal(job!.lastError, null)

    const events = await f.events(processInstanceId, 'job.requeued')
    assert.equal(events.length, 1, 'operator requeue is audited')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14F: requeueJob refuses non-failed jobs', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })
    const { processInstanceId } = await startTimerInstance(f, engine)
    const jobId = (await f.jobs(processInstanceId))[0].id

    await assert.rejects(
      () => engine.requeueJob({ jobId, actor: 'op' }),
      (e: any) => e?.code === 'JOB_NOT_REQUEUEABLE',
    )
  } finally {
    await f.cleanup()
  }
})
test('CRM-14F: runDueJobs advances a due timer exactly once (bounded poller)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })
    const { processInstanceId } = await startTimerInstance(f, engine)

    const res = await engine.runDueJobs('poller', 10)
    assert.equal(res.fired, 1, 'one due timer fired')
    assert.equal(res.failed, 0)

    const jobs = await f.jobs(processInstanceId)
    assert.equal(jobs[0].status, 'completed')
    const tokens = await f.tokens(processInstanceId)
    assert.equal(tokens.filter((t: any) => t.node_id === 'end').length, 1)
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')

    // Duplicate fire rejected — second pass has no work.
    const res2 = await engine.runDueJobs('poller', 10)
    assert.equal(res2.claimed.length, 0)
  } finally {
    await f.cleanup()
  }
})

test('CRM-14F: runDueJobs bounds the batch and fails unexecutable jobs persistently', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })
    const { processInstanceId } = await startTimerInstance(f, engine)

    // Create a generic async job for the no-executor path. maxAttempts=1 so a
    // single failure is terminal 'failed' (persistent, operator-recoverable).
    const asyncJobId = await engine.createJob({
      processInstanceId,
      tenantId: f.tenantId,
      type: 'async',
      dueAt: new Date(T0.getTime()),
      maxAttempts: 1,
    })
    const res = await engine.runDueJobs('poller', 10)
    assert.equal(res.failed, 1, 'no-executor job fails persistently')
    const job = await engine.getJob(asyncJobId)
    assert.equal(job!.status, 'failed')
    assert.match(job!.lastError ?? '', /no executor/)
  } finally {
    await f.cleanup()
  }
})


test('CRM-14F: terminal process leaves no pending/locked job (deadline cleanup)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })
    const { processInstanceId } = await startTimerInstance(f, engine)
    await engine.cancelProcess({ processInstanceId, actor: 'operator', reason: 'test' })

    const jobs = await f.jobs(processInstanceId)
    assert.ok(jobs.length >= 1)
    for (const j of jobs) {
      assert.equal(j.status, 'cancelled', 'no pending/locked job survives termination')
    }
    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'aborted')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14F: two concurrent terminalization contenders produce one terminal result', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_lease', 1, TIMER_END)
    const engine = f.makeEngine({ now: () => new Date(T0.getTime() + 10_000) })
    const { processInstanceId } = await startTimerInstance(f, engine)

    // Two concurrent cancel attempts on the same active instance: exactly one
    // terminal transition (the second observes the terminal state).
    await Promise.all([
      engine.cancelProcess({ processInstanceId, actor: 'a', reason: 'x' }),
      engine.cancelProcess({ processInstanceId, actor: 'b', reason: 'y' }),
    ])

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'aborted')
    const cancels = await f.events(processInstanceId, 'process.cancelled')
    assert.equal(cancels.length, 1, 'exactly one process.cancelled event')
  } finally {
    await f.cleanup()
  }
})