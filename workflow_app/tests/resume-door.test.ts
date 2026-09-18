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
  stops: Array<
    | {
        taskId: string
        nodeId: string
        claimedAt?: string | null
        forkChild?: boolean
        openSiblings?: number
      }
    | null
  >
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

// ---------------------------------------------------------------------------
// ENG-FORGE-SPLIT-SIBLING-01 — THE DOOR MUST NOT FABRICATE A COMPLETION.
//
// The door moves a run that is PARKED. A dynamic-fork branch that has never been claimed is not
// parked: no lane ran for it, no work item exists for it, no proof was attempted. Advancing it
// marks a sibling's work done and the fork can never recover — measured live on 2026-09-18, when a
// resume completed branch 0 of 2 (`claimed_at null, completed_by operator`) and every later run
// could only reach branch 1 while lead_post refused the join as "never reached a terminal state".
// These fences assert the refusal AND that it does not over-block real work.
// ---------------------------------------------------------------------------

test('split-sibling: an unstarted fork branch is refused by name, and nothing is written', async () => {
  const { engine, seen } = fakeEngine({
    instanceId: 'inst-branch',
    stops: [
      {
        taskId: 't-branch-0',
        nodeId: 'smith_split_work',
        claimedAt: null,
        forkChild: true,
        openSiblings: 1,
      },
    ],
  })

  const result = await resolveForgeHold(
    { storyId: 'S', resolution: 'resolve', resumeTarget: 'SMITH', resolver: 'operator' },
    engine,
  )

  assert.equal(result.outcome, 'refused')
  const missing = result.outcome === 'refused' ? result.missing : ''
  assert.match(missing, /never been claimed/)
  assert.match(missing, /1 sibling branch/)
  assert.match(missing, /--cancel/, 'the honest alternative must be named')
  assert.deepEqual(seen.completed, [], 'an unstarted branch must not be advanced')
  assert.deepEqual(seen.records, [], 'a refusal writes no resolution row')
})

test('split-sibling: a fork branch that WAS claimed still moves — real parked work is not blocked', async () => {
  const { engine, seen } = fakeEngine({
    instanceId: 'inst-branch-claimed',
    stops: [
      {
        taskId: 't-branch-1',
        nodeId: 'smith_split_work',
        claimedAt: '2026-09-18T12:47:22Z',
        forkChild: true,
        openSiblings: 1,
      },
      { taskId: 't-hold', nodeId: 'hold' },
    ],
  })

  const result = await resolveForgeHold(
    { storyId: 'S', resolution: 'resolve', resumeTarget: 'QA', resolver: 'operator' },
    engine,
  )

  assert.equal(result.outcome, 'resolved')
  assert.equal(seen.completed.length, 2, 'the claimed branch advances to the hold gate, then resolves')
})

test('split-sibling: an unclaimed SERIAL lane task is still movable — the guard is about forks', async () => {
  const { engine, seen } = fakeEngine({
    instanceId: 'inst-serial',
    stops: [
      { taskId: 't-lead-pre', nodeId: 'lead_pre', claimedAt: null, forkChild: false, openSiblings: 0 },
      { taskId: 't-hold', nodeId: 'hold' },
    ],
  })

  const result = await resolveForgeHold(
    { storyId: 'S', resolution: 'resolve', resumeTarget: 'SMITH', resolver: 'operator' },
    engine,
  )

  assert.equal(result.outcome, 'resolved')
  assert.equal(seen.completed[0].taskId, 't-lead-pre')
})

test('split-sibling: cancel is still allowed on an unstarted branch, because terminating is honest', async () => {
  const { engine, seen } = fakeEngine({
    instanceId: 'inst-cancel',
    stops: [
      {
        taskId: 't-branch-0',
        nodeId: 'smith_split_work',
        claimedAt: null,
        forkChild: true,
        openSiblings: 1,
      },
    ],
  })

  const result = await resolveForgeHold(
    { storyId: 'S', resolution: 'cancel', resolver: 'operator', reason: 'wrong decomposition' },
    engine,
  )

  assert.equal(result.outcome, 'resolved')
  assert.equal(seen.cancelled.length, 1)
  assert.deepEqual(seen.completed, [])
})



test('split-sibling: an unclaimed COORDINATION task on a branch token is movable — only the WORK branch is protected', async () => {
  // Narrowed AFTER the first version over-blocked: forkChild is inherited by downstream tasks that
  // ride a branch token, and parking a lead_post at the hold gate fabricates no work. Measured live
  // when the probe returned exactly this shape and the guard would have refused an ordinary resume.
  const { engine, seen } = fakeEngine({
    instanceId: 'inst-lead-post-branch',
    stops: [
      { taskId: 't-lead-post', nodeId: 'lead_post', claimedAt: null, forkChild: true, openSiblings: 0 },
      { taskId: 't-hold', nodeId: 'hold' },
    ],
  })

  const result = await resolveForgeHold(
    { storyId: 'S', resolution: 'resolve', resumeTarget: 'SMITH', resolver: 'operator' },
    engine,
  )

  assert.equal(result.outcome, 'resolved')
  assert.equal(seen.completed[0].taskId, 't-lead-post')
})
