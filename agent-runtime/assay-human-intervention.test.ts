import assert from 'node:assert/strict'
import test from 'node:test'

import { assayInterruptionRequiresHuman } from '../db/agent-work-recovery'
import { followFinishedLane } from './orchestrate-apply'

test('Assay runtime interruption is a human gate regardless of retry budget role', () => {
  assert.equal(assayInterruptionRequiresHuman('verifier'), true)
  assert.equal(assayInterruptionRequiresHuman('reviewer'), true)
  assert.equal(assayInterruptionRequiresHuman('VERIFIER'), true)
  assert.equal(assayInterruptionRequiresHuman('builder'), false)
  assert.equal(assayInterruptionRequiresHuman('scout'), false)
  assert.equal(assayInterruptionRequiresHuman(null), false)
})

for (const finishedRole of ['verifier', 'reviewer']) {
  test(`failed ${finishedRole} Assay is terminal for automation: no hydration and no Smith enqueue`, async () => {
    let storyRead = false
    const enqueued: unknown[] = []

    const followed = await followFinishedLane({
      storyId: 'ENG-FORGE-ASSAY-HUMAN-GATE',
      finishedRole,
      resultStatus: 'Complete',
      testsSummary: 'exit 1; tests 26; pass 25; fail 1',
      getStory: async () => {
        storyRead = true
        return {
          architectBrief: 'This must never be consulted after a failed Assay.',
          acceptanceCriteria: 'No automatic repair.',
          assayCommands: 'pnpm exec tsx --test agent-runtime/example.test.ts',
        }
      },
      enqueue: async (input) => {
        enqueued.push(input)
      },
    })

    assert.equal(followed, null)
    assert.equal(storyRead, false, 'failed Assay stops before any lane-selection hydration')
    assert.deepEqual(enqueued, [], 'failed Assay cannot enqueue Smith or any other lane')
  })
}

test('non-Complete Assay is also a human gate even when test prose looks clean', async () => {
  let storyRead = false
  const enqueued: unknown[] = []

  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-ASSAY-HUMAN-GATE-STATUS',
    finishedRole: 'verifier',
    resultStatus: 'Hold',
    testsSummary: 'exit 0; tests 26; pass 26; fail 0',
    getStory: async () => {
      storyRead = true
      return {}
    },
    enqueue: async (input) => {
      enqueued.push(input)
    },
  })

  assert.equal(followed, null)
  assert.equal(storyRead, false)
  assert.deepEqual(enqueued, [])
})
