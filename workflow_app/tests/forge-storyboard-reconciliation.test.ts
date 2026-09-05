import assert from 'node:assert/strict'
import test from 'node:test'

import { projectForgeStoryboardState } from '../forge/forge-engine-runtime'

test('ENG-FORGE-V10: active engine truth projects to In Progress or explicit Hold', () => {
  assert.deepEqual(projectForgeStoryboardState({ status: 'active', outcome: null }), {
    state: 'in_progress',
  })
  assert.equal(
    projectForgeStoryboardState({ status: 'active', outcome: null, humanHold: true }).state,
    'hold',
  )
})

test('ENG-FORGE-V10: successful terminal engine truth alone projects Complete', () => {
  assert.deepEqual(projectForgeStoryboardState({ status: 'completed', outcome: 'completed' }), {
    state: 'complete',
  })
  assert.equal(
    projectForgeStoryboardState({ status: 'completed', outcome: 'failed' }).state,
    'failed',
  )
})

test('ENG-FORGE-V10: engine error and aborted statuses cannot regress to In Progress', () => {
  assert.equal(projectForgeStoryboardState({ status: 'error', outcome: 'failed' }).state, 'failed')
  assert.equal(projectForgeStoryboardState({ status: 'aborted', outcome: 'cancelled' }).state, 'hold')
})
