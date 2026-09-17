import assert from 'node:assert/strict'
import test from 'node:test'

import type { ForgeHoldResolutionInput } from '../../db/forge-hold'
import { resolveForgeHold, type ForgeResumeEngine } from '../forge/forge-hold-resolve'

// ---------------------------------------------------------------------------
// ENG-FORGE-RESUME-DOOR-01 — the door asserts the DECISION SEAM with an injected
// engine, never a live engine instance (Architect risk: a proof that drives the
// real engine needs a seeded instance). It asserts which task is moved, on which
// transition, with what evidence, and what durable row is written — or, when the
// run cannot be moved, that the missing piece is NAMED and no row is written.
// ---------------------------------------------------------------------------

type Seen = {
  completed: Array<{ taskId: string; transitionName: string; evidence: Record<string, unknown> }>
  cancelled: Array<{ instanceId: string; actor: string; reason?: string }>
  records: ForgeHoldResolutionInput[]
}

function fakeEngine(state: {
  instanceId: string | null
  stops: Array<{ taskId: string; nodeId: string } | null>
}): { engine: ForgeResumeEngine; seen: Seen } {
  const seen: Seen = { completed: [], cancelled: [], records: [] }
  let call = 0
  const engine: ForgeResumeEngine = {
    findActiveInstance: async () => state.instanceId,
    findStopPoint: async () =>
      state.stops[Math.min(call++, state.stops.length - 1)] ?? null,
    completeTask: async (taskId, opts) => {
      seen.completed.push({
        taskId,
        transitionName: opts.transitionName,
        evidence: { ...opts.evidence },
      })
    },
    cancelInstance: async (instanceId, opts) => {
      seen.cancelled.push({ instanceId, actor: opts.actor, reason: opts.reason })
    },
    appendHoldRecord: async (input) => {
      seen.records.push(input)
      return seen.records.length
    },
  }
  return { engine, seen }
}

test('resume-door: a held run resumes at a named node', async () => {
  const { engine, seen } = fakeEngine({
    instanceId: 'inst-1',
    stops: [{ taskId: 't-hold', nodeId: 'hold' }],
  })

  const result = await resolveForgeHold(
    { storyId: 'S', resolution: 'resolve', resumeTarget: 'QA', resolver: 'chris', reason: 'qa re-run' },
    engine,
  )

  assert.equal(result.outcome, 'resolved')
  assert.equal(result.stopNode, 'hold')
  assert.equal(result.movedTo, 'QA')
  assert.deepEqual(seen.completed, [
    { taskId: 't-hold', transitionName: 'resolve', evidence: { resumeTarget: 'QA' } },
  ])
  assert.equal(seen.records.length, 1)
})

test('resume-door: a failed lane run with no hold task resumes at a named node', async () => {
  const { engine, seen } = fakeEngine({
    instanceId: 'inst-2',
    stops: [
      { taskId: 't-smith', nodeId: 'smith' },
      { taskId: 't-hold', nodeId: 'hold' },
    ],
  })

  const result = await resolveForgeHold(
    { storyId: 'S', resolution: 'resolve', resumeTarget: 'QA', resolver: 'chris' },
    engine,
  )

  assert.equal(result.outcome, 'resolved')
  assert.equal(result.stopNode, 'smith')
  assert.equal(result.movedTo, 'QA')
  assert.deepEqual(seen.completed[0], { taskId: 't-smith', transitionName: 'hold', evidence: {} })
  assert.deepEqual(seen.completed[1], {
    taskId: 't-hold',
    transitionName: 'resolve',
    evidence: { resumeTarget: 'QA' },
  })
  assert.equal(seen.records.length, 1)
  assert.equal(seen.records[0].originatingNode, 'smith')
})

test('resume-door: a failed lane run with no hold task cancels', async () => {
  const { engine, seen } = fakeEngine({
    instanceId: 'inst-3',
    stops: [{ taskId: 't-smith', nodeId: 'smith' }],
  })

  const result = await resolveForgeHold(
    { storyId: 'S', resolution: 'cancel', resolver: 'chris', reason: 'dead lane' },
    engine,
  )

  assert.equal(result.outcome, 'resolved')
  assert.equal(result.movedTo, null)
  assert.deepEqual(seen.completed, [])
  assert.deepEqual(seen.cancelled, [{ instanceId: 'inst-3', actor: 'chris', reason: 'dead lane' }])
  assert.equal(seen.records.length, 1)
  assert.equal(seen.records[0].resolution, 'cancel')
})

test('resume-door: resolution row records resolver, time and reason', async () => {
  const { engine, seen } = fakeEngine({
    instanceId: 'inst-4',
    stops: [{ taskId: 't-smith', nodeId: 'smith' }],
  })

  const result = await resolveForgeHold(
    {
      storyId: 'S',
      resolution: 'cancel',
      resolver: 'chris',
      reason: 'lane errored, clearing it',
    },
    engine,
  )

  assert.equal(result.outcome, 'resolved')
  assert.equal(typeof result.auditId, 'number')
  const row = seen.records[0]
  assert.equal(row.resolver, 'chris')
  assert.equal(row.reason, 'lane errored, clearing it')
  assert.equal(row.resolution, 'cancel')
  assert.equal(row.storyId, 'S')
  assert.equal(row.processInstanceId, 'inst-4')
  assert.equal(row.taskId, 't-smith')
  assert.equal(row.originatingNode, 'smith')
})

test('resume-door: an unresumable run names what is missing', async () => {
  const terminal = fakeEngine({ instanceId: null, stops: [] })
  const refused = await resolveForgeHold(
    { storyId: 'S', resolution: 'cancel', resolver: 'chris' },
    terminal.engine,
  )
  assert.equal(refused.outcome, 'refused')
  assert.match(refused.missing, /no active FORGE_SDLC instance/)
  assert.equal(terminal.seen.records.length, 0)

  const noTask = fakeEngine({ instanceId: 'inst-x', stops: [null] })
  const refused2 = await resolveForgeHold(
    { storyId: 'S', resolution: 'cancel', resolver: 'chris' },
    noTask.engine,
  )
  assert.equal(refused2.outcome, 'refused')
  assert.match(refused2.missing, /no open engine task/)
  assert.equal(noTask.seen.records.length, 0)
})

test('resume-door: resume writes no verdict', async () => {
  const { engine, seen } = fakeEngine({
    instanceId: 'inst-6',
    stops: [
      { taskId: 't-smith', nodeId: 'smith' },
      { taskId: 't-hold', nodeId: 'hold' },
    ],
  })

  await resolveForgeHold(
    { storyId: 'S', resolution: 'resolve', resumeTarget: 'ARCHITECT', resolver: 'chris' },
    engine,
  )

  assert.deepEqual(seen.completed[0].evidence, {})
  assert.deepEqual(Object.keys(seen.completed[1].evidence), ['resumeTarget'])
  for (const done of seen.completed) {
    assert.equal('leadDecision' in done.evidence, false)
    assert.equal('qaPassed' in done.evidence, false)
    assert.equal('failureClass' in done.evidence, false)
  }
})
