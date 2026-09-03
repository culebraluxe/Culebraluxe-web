import assert from 'node:assert/strict'
import test from 'node:test'

import {
  assayFailureEvidence,
  followFinishedLane,
  hydrateBareReadyItems,
  isCleanAssayResult,
} from './orchestrate-apply'

const story = {
  architectBrief: 'Implement the existing Forge packet only.',
  goal: 'Keep Assay failure in repair.',
}

test('Assay Complete with clean tests is a clean pass', () => {
  assert.equal(
    isCleanAssayResult({
      resultStatus: 'Complete',
      testsSummary: '12 passed, 0 errors',
    }),
    true,
  )
})

test('Assay non-Complete result fails closed', () => {
  assert.equal(
    isCleanAssayResult({
      resultStatus: 'Partial',
      testsSummary: 'tests executed',
    }),
    false,
  )
})

test('Assay nominal Complete with fail violation or policy evidence fails closed', () => {
  for (const testsSummary of [
    '1 failed, 11 passed',
    'verification violation detected',
    'policy gate rejected the change',
  ]) {
    assert.equal(
      isCleanAssayResult({ resultStatus: 'Complete', testsSummary }),
      false,
      testsSummary,
    )
  }
})

test('Assay failure evidence retains packet-named failed commands', () => {
  assert.equal(
    assayFailureEvidence({
      testsSummary: '1 failed',
      failedCommands: ['node --test agent-runtime/orchestrate-apply.test.ts', 'pnpm typecheck'],
    }),
    '1 failed | failed commands: node --test agent-runtime/orchestrate-apply.test.ts, pnpm typecheck',
  )
})

test('Assay fail never enqueues grow', async () => {
  const enqueued: unknown[] = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V3-01',
    finishedRole: 'reviewer',
    resultStatus: 'Complete',
    testsSummary: 'policy violation',
    getStory: async () => story,
    enqueue: async (input) => {
      enqueued.push(input)
    },
  })

  assert.equal(followed, null)
  assert.deepEqual(enqueued, [])
})

test('Assay clean pass preserves current terminal follow behavior', async () => {
  const enqueued: unknown[] = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V3-01',
    finishedRole: 'reviewer',
    resultStatus: 'Complete',
    testsSummary: 'all checks passed',
    getStory: async () => story,
    enqueue: async (input) => {
      enqueued.push(input)
    },
  })

  assert.equal(followed, null)
  assert.deepEqual(enqueued, [])
})

test('Ready with no Neon brief and no git packet hydrates Scout, never Smith', async () => {
  const enqueued: Array<{ role: string; modelProfile: string }> = []
  const stamped = await hydrateBareReadyItems({
    listItems: async () => [{
      id: 'work-1',
      storyId: 'ENG-FORGE-V3-02-NO-BRIEF',
      state: 'Ready',
      role: null,
      modelProfile: null,
      executionEnvironment: 'DEV',
      executionPolicy: 'Unattended OK',
      priority: 1,
    }],
    getStory: async () => ({ goal: 'needs architecture first' }),
    enqueue: async (input) => {
      enqueued.push({ role: input.role, modelProfile: input.modelProfile })
    },
    repoRoot: '/definitely/missing',
  })

  assert.deepEqual(stamped, ['ENG-FORGE-V3-02-NO-BRIEF:scout'])
  assert.equal(enqueued.length, 1)
  assert.equal(enqueued[0]?.role, 'scout')
  assert.equal(enqueued[0]?.modelProfile, 'scout-volume')
})

test('Scout done with no brief stops: no second Scout and no Smith', async () => {
  const enqueued: unknown[] = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V3-03-NO-BRIEF',
    finishedRole: 'scout',
    resultStatus: 'Complete',
    getStory: async () => ({ goal: 'still needs architecture' }),
    enqueue: async (input) => {
      enqueued.push(input)
    },
    repoRoot: '/definitely/missing',
  })

  assert.equal(followed, null)
  assert.deepEqual(enqueued, [])
})

test('Scout done with brief present follows to Smith', async () => {
  const enqueued: Array<{ role: string; modelProfile: string }> = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V3-03-WITH-BRIEF',
    finishedRole: 'scout',
    resultStatus: 'Complete',
    getStory: async () => ({ architectBrief: 'Build only the approved slice.' }),
    enqueue: async (input) => {
      enqueued.push({ role: input.role, modelProfile: input.modelProfile })
    },
    repoRoot: '/definitely/missing',
  })

  assert.equal(followed, 'smith')
  assert.equal(enqueued.length, 1)
  assert.equal(enqueued[0]?.role, 'builder')
})
