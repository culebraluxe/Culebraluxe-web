import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  reconcileDeadlineTimerCore,
  type DeadlineTimerDeps,
} from '../deadline-timer'

// ---------------------------------------------------------------------------
// CRM-22 — generic deadline-timer reconciliation core (pure, deterministic).
//
// The core never invents a deadline: no canonical date or no pending timer
// => nothing happens. When a canonical date changes, the SAME pending timer
// job is rescheduled (no duplicate, no instance restart). Lookups are scoped
// by the timer node id (job payload.nodeId), so one instance with several
// pending milestone timers (closing / inspection / financing) stays
// deterministic per milestone.
// ---------------------------------------------------------------------------

type Timer = { jobId: string; dueAt: string; nodeId: string }

function makeDeps(initial: Timer[] = []) {
  const timers: Timer[] = [...initial]
  const calls: string[] = []
  const deps: DeadlineTimerDeps = {
    findPendingTimer: async (instanceId, nodeId) => {
      const t = timers.find((x) => x.nodeId === nodeId)
      return t ? { jobId: t.jobId, dueAt: t.dueAt } : null
    },
    reschedule: async (jobId, dueAt) => {
      calls.push('reschedule')
      const t = timers.find((x) => x.jobId === jobId)
      if (t) t.dueAt = dueAt.toISOString()
    },
  }
  return { deps, calls, timers }
}

test('A. no pending timer leaves nothing to reschedule (the XML node schedules it)', async () => {
  const { deps, timers } = makeDeps([])
  const res = await reconcileDeadlineTimerCore(
    'inst-1',
    'inspection_deadline_timer',
    '2026-09-01T00:00:00.000Z',
    deps,
  )
  assert.equal(res.action, 'unchanged')
  assert.equal(timers.length, 0)
})

test('B. deadline extension reschedules the SAME pending timer', async () => {
  const { deps, timers, calls } = makeDeps([
    { jobId: 'job-1', dueAt: '2026-09-01T00:00:00.000Z', nodeId: 'inspection_deadline_timer' },
  ])
  const res = await reconcileDeadlineTimerCore(
    'inst-1',
    'inspection_deadline_timer',
    '2026-10-01T00:00:00.000Z',
    deps,
  )
  assert.equal(res.action, 'rescheduled')
  assert.deepEqual(calls, ['reschedule'])
  assert.equal(timers[0].jobId, 'job-1', 'the same job is reused — no duplicate timer')
  assert.equal(timers[0].dueAt, '2026-10-01T00:00:00.000Z')
})

test('C. earlier deadline reschedules the timer', async () => {
  const { deps, timers, calls } = makeDeps([
    { jobId: 'job-1', dueAt: '2026-10-01T00:00:00.000Z', nodeId: 'financing_deadline_timer' },
  ])
  const res = await reconcileDeadlineTimerCore(
    'inst-1',
    'financing_deadline_timer',
    '2026-09-01T00:00:00.000Z',
    deps,
  )
  assert.equal(res.action, 'rescheduled')
  assert.deepEqual(calls, ['reschedule'])
  assert.equal(timers[0].dueAt, '2026-09-01T00:00:00.000Z')
})

test('D. unchanged deadline leaves the timer untouched', async () => {
  const { deps, calls, timers } = makeDeps([
    { jobId: 'job-1', dueAt: '2026-09-01T00:00:00.000Z', nodeId: 'inspection_deadline_timer' },
  ])
  const res = await reconcileDeadlineTimerCore(
    'inst-1',
    'inspection_deadline_timer',
    '2026-09-01T00:00:00.000Z',
    deps,
  )
  assert.equal(res.action, 'unchanged')
  assert.deepEqual(calls, [])
  assert.equal(timers[0].jobId, 'job-1')
})

test('E. no canonical deadline invents nothing', async () => {
  const { deps, timers } = makeDeps([
    { jobId: 'job-1', dueAt: '2026-09-01T00:00:00.000Z', nodeId: 'inspection_deadline_timer' },
  ])
  const res = await reconcileDeadlineTimerCore(
    'inst-1',
    'inspection_deadline_timer',
    null,
    deps,
  )
  assert.equal(res.action, 'unchanged')
  assert.equal(timers[0].jobId, 'job-1')
  assert.equal(timers[0].dueAt, '2026-09-01T00:00:00.000Z', 'existing timer is left alone')
})

test('F. sequential extensions reuse the SAME timer job (no duplicate timer)', async () => {
  const { deps, timers, calls } = makeDeps([
    { jobId: 'job-1', dueAt: '2026-09-01T00:00:00.000Z', nodeId: 'inspection_deadline_timer' },
  ])
  await reconcileDeadlineTimerCore('inst-1', 'inspection_deadline_timer', '2026-10-01T00:00:00.000Z', deps)
  await reconcileDeadlineTimerCore('inst-1', 'inspection_deadline_timer', '2026-11-01T00:00:00.000Z', deps)
  assert.deepEqual(calls, ['reschedule', 'reschedule'])
  assert.equal(timers[0].jobId, 'job-1')
  assert.equal(timers[0].dueAt, '2026-11-01T00:00:00.000Z')
})

test('G. timers are disambiguated by milestone node id (multi-timer instance)', async () => {
  const { deps, timers } = makeDeps([
    { jobId: 'job-inspection', dueAt: '2026-09-01T00:00:00.000Z', nodeId: 'inspection_deadline_timer' },
    { jobId: 'job-financing', dueAt: '2026-09-01T00:00:00.000Z', nodeId: 'financing_deadline_timer' },
  ])
  // Amending only the financing deadline must reschedule ONLY the financing
  // timer; the inspection timer is untouched.
  const res = await reconcileDeadlineTimerCore(
    'inst-1',
    'financing_deadline_timer',
    '2026-12-01T00:00:00.000Z',
    deps,
  )
  assert.equal(res.action, 'rescheduled')
  assert.equal(res.jobId, 'job-financing')
  assert.equal(timers.find((t) => t.jobId === 'job-financing')!.dueAt, '2026-12-01T00:00:00.000Z')
  assert.equal(timers.find((t) => t.jobId === 'job-inspection')!.dueAt, '2026-09-01T00:00:00.000Z')
})

test('H. the workflow instance identity is never changed by deadline reconciliation', async () => {
  const { deps } = makeDeps([
    { jobId: 'job-1', dueAt: '2026-09-01T00:00:00.000Z', nodeId: 'inspection_deadline_timer' },
  ])
  const res = await reconcileDeadlineTimerCore(
    'inst-42',
    'inspection_deadline_timer',
    '2026-10-01T00:00:00.000Z',
    deps,
  )
  // The core only reschedules jobs; the instance id is untouched.
  assert.equal(res.jobId, 'job-1')
})
