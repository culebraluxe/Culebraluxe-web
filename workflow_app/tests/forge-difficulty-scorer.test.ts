import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  DIFFICULTY_WEIGHTS,
  difficultyGate,
  FLAG_BELOW,
  LogisticScorer,
  NEUTRAL_HISTORY,
  REJECT_BELOW,
  scoreDifficulty,
  sigmoid,
  type DifficultyFeatures,
} from '../forge/forge-difficulty-scorer'

const scorer = new LogisticScorer()

const features = (partial: Partial<DifficultyFeatures>): DifficultyFeatures => ({
  filesTouched: 1,
  locRatio: 0.2,
  depDepth: 1,
  hasAcceptance: true,
  contextRatio: 0.3,
  historicalSuccess: NEUTRAL_HISTORY,
  repoSizeBucket: 1,
  genericType: false,
  ...partial,
})

test('difficulty scorer: a small well-shaped leaf with an acceptance signal dispatches', () => {
  const { pSuccess, gate } = scoreDifficulty(scorer, features({}))
  assert.ok(pSuccess >= FLAG_BELOW, `expected dispatch, got p=${pSuccess.toFixed(3)}`)
  assert.equal(gate, 'dispatch')
})

test('difficulty scorer: a 7-file heavy leaf is rejected (SWE-bench ~0% shape)', () => {
  const { pSuccess, gate } = scoreDifficulty(
    scorer,
    features({ filesTouched: 7, locRatio: 1.0, contextRatio: 0.9 }),
  )
  assert.ok(pSuccess < REJECT_BELOW, `expected reject, got p=${pSuccess.toFixed(3)}`)
  assert.equal(gate, 'reject')
})

test('difficulty scorer: a missing acceptance signal pushes a borderline leaf down a gate', () => {
  const base = { filesTouched: 3, locRatio: 0.7, depDepth: 2, contextRatio: 0.6 }
  const withAcc = scoreDifficulty(scorer, features({ ...base, hasAcceptance: true }))
  const noAcc = scoreDifficulty(scorer, features({ ...base, hasAcceptance: false }))
  assert.ok(noAcc.pSuccess < withAcc.pSuccess, 'acceptance signal must raise p(success)')
  assert.ok(
    noAcc.pSuccess < FLAG_BELOW,
    `borderline leaf without acceptance must be below dispatch, got ${noAcc.pSuccess.toFixed(3)}`,
  )
})

test('difficulty scorer: history is the strongest signal (neutral vs perfect fit)', () => {
  const neutral = scoreDifficulty(scorer, features({ filesTouched: 3, locRatio: 0.6, contextRatio: 0.6 }))
  const known = scoreDifficulty(
    scorer,
    features({ filesTouched: 3, locRatio: 0.6, contextRatio: 0.6, historicalSuccess: 0.9 }),
  )
  assert.ok(known.pSuccess > neutral.pSuccess)
})

test('difficulty scorer: sigmoid is bounded and the seam supports a learned swap', () => {
  assert.ok(sigmoid(-1e9) < 1e-20, 'large negative clamps to ~0')
  assert.equal(sigmoid(1e9), 1)
  // A learned scorer implementing the same seam swaps in without call-site change.
  const learned = { score: () => 0.9 }
  assert.equal(scoreDifficulty(learned, features({})).gate, 'dispatch')
  assert.equal(DIFFICULTY_WEIGHTS.historicalSuccess, 2.2)
})
