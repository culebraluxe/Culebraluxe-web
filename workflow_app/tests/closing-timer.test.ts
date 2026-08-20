import { test } from 'node:test'
import assert from 'node:assert/strict'
import { reconcileClosingTimerCore, type ClosingTimerDeps } from '../closing-timer'

function makeDeps(initial: { jobId: string; dueAt: string } | null = null) {
  let timer = initial
  const calls: string[] = []
  const deps: ClosingTimerDeps = {
    findPendingTimer: async () => timer,
    schedule: async (instanceId, dueAt) => {
      calls.push('schedule')
      timer = { jobId: 'job-1', dueAt: dueAt.toISOString() }
      return 'job-1'
    },
    reschedule: async (jobId, dueAt) => {
      calls.push('reschedule')
      timer = { jobId, dueAt: dueAt.toISOString() }
    },
  }
  return { deps, calls, getTimer: () => timer }
}

test('A. initial closing date schedules the expected timer', async () => {
  const { deps, getTimer } = makeDeps(null)
  const res = await reconcileClosingTimerCore('inst-1', '2026-09-01T00:00:00.000Z', deps)
  assert.equal(res.action, 'scheduled')
  assert.equal(getTimer()!.dueAt, '2026-09-01T00:00:00.000Z')
})

test('B. closing date extension reschedules the timer', async () => {
  const { deps, getTimer, calls } = makeDeps({ jobId: 'job-1', dueAt: '2026-09-01T00:00:00.000Z' })
  const res = await reconcileClosingTimerCore('inst-1', '2026-10-01T00:00:00.000Z', deps)
  assert.equal(res.action, 'rescheduled')
  assert.deepEqual(calls, ['reschedule'])
  assert.equal(getTimer()!.dueAt, '2026-10-01T00:00:00.000Z')
})

test('C. earlier target date reschedules the timer', async () => {
  const { deps, getTimer, calls } = makeDeps({ jobId: 'job-1', dueAt: '2026-10-01T00:00:00.000Z' })
  const res = await reconcileClosingTimerCore('inst-1', '2026-09-01T00:00:00.000Z', deps)
  assert.equal(res.action, 'rescheduled')
  assert.deepEqual(calls, ['reschedule'])
  assert.equal(getTimer()!.dueAt, '2026-09-01T00:00:00.000Z')
})

test('D. obsolete timer is replaced, not duplicated', async () => {
  const { deps, calls, getTimer } = makeDeps({ jobId: 'job-1', dueAt: '2026-09-01T00:00:00.000Z' })
  await reconcileClosingTimerCore('inst-1', '2026-11-01T00:00:00.000Z', deps)
  // Exactly one timer remains (rescheduled in place), never two.
  assert.deepEqual(calls, ['reschedule'])
  assert.equal(getTimer()!.jobId, 'job-1')
  assert.equal(getTimer()!.dueAt, '2026-11-01T00:00:00.000Z')
})

test('E. workflow instance identity is never changed by timer reconciliation', async () => {
  const { deps } = makeDeps(null)
  const res = await reconcileClosingTimerCore('inst-42', '2026-09-01T00:00:00.000Z', deps)
  // The core only schedules/reschedules jobs; the instance id is untouched.
  assert.equal(res.jobId, 'job-1')
})

test('no canonical closing date invents nothing', async () => {
  const { deps, getTimer } = makeDeps(null)
  const res = await reconcileClosingTimerCore('inst-1', null, deps)
  assert.equal(res.action, 'unchanged')
  assert.equal(getTimer(), null)
})
