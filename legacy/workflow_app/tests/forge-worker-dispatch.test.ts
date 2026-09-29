import assert from 'node:assert/strict'
import test from 'node:test'

import { chooseWorkerDispatch, engineWorkTypeForKind } from '@/legacy/workflow_app/forge/worker-dispatch'
import type { ForgeNightPlan } from '@/legacy/workflow_app/forge/forge-night-driver'

// ---------------------------------------------------------------------------
// WHICH PATH DRIVES A WORKER PASS.
//
// The flag `driveEngine` had no reader, so a Ready story waited for a human to run
// `pnpm forge:engine --story …` by hand while the worker kept ticking the lane path
// that cannot finish a story. These tests are the seam: they fix WHICH path is
// chosen, and they are the reason the choice is a pure function rather than a
// conditional buried in the worker entry.
// ---------------------------------------------------------------------------

const engineBrain: ForgeNightPlan = { brain: 'engine', hydrate: false, follow: false, publish: false, driveEngine: true }
const reducerBrain: ForgeNightPlan = { brain: 'reducer', hydrate: true, follow: true, publish: true, driveEngine: false }

test('the engine brain drives the OLDEST ready story through the engine', () => {
  const dispatch = chooseWorkerDispatch({
    plan: engineBrain,
    ready: [
      { storyId: 'ENG-B', queuedAt: '2026-09-15T20:00:00Z', kind: 'feature' },
      { storyId: 'ENG-A', queuedAt: '2026-09-15T19:00:00Z', kind: 'fix' },
    ],
  })
  assert.equal(dispatch.kind, 'engine')
  assert.equal(dispatch.kind === 'engine' && dispatch.storyId, 'ENG-A', 'FIFO: the oldest item runs first')
  assert.equal(dispatch.kind === 'engine' && dispatch.workType, 'BUG', 'a fix is the bug path')
})

test('the reducer brain keeps the lane path — the engine must not be a second writer', () => {
  const dispatch = chooseWorkerDispatch({
    plan: reducerBrain,
    ready: [{ storyId: 'ENG-A', queuedAt: '2026-09-15T19:00:00Z', kind: 'feature' }],
  })
  assert.equal(dispatch.kind, 'lane')
  assert.match(dispatch.reason, /reducer path owns this pass/)
})

test('an idle queue takes the lane path so the pass reports idle', () => {
  const dispatch = chooseWorkerDispatch({ plan: engineBrain, ready: [] })
  assert.equal(dispatch.kind, 'lane')
  assert.match(dispatch.reason, /no Ready work item/)
})

test('kind maps to the engine work type, and an unrecorded kind takes the default path', () => {
  assert.equal(engineWorkTypeForKind('fix'), 'BUG')
  assert.equal(engineWorkTypeForKind('QA'), 'RESEARCH')
  assert.equal(engineWorkTypeForKind('learn'), 'RESEARCH')
  assert.equal(engineWorkTypeForKind('feature'), 'FEATURE')
  assert.equal(engineWorkTypeForKind('judgment'), 'FEATURE')
  assert.equal(engineWorkTypeForKind(null), 'FEATURE')
  assert.equal(engineWorkTypeForKind('something-new'), 'FEATURE')
})
