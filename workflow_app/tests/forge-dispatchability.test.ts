import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  ANTI_TOKEN_FIRE,
  dispatchabilityFor,
  isDispatchable,
  MAX_CHUNKS_PER_STORY,
  type DispatchabilityFeatures,
} from '../forge/forge-dispatchability'

const LOW: DispatchabilityFeatures = {
  semanticSurface: 1,
  dependencyDepth: 1,
  uncertainty: 1,
  contextBurden: 1,
  proofBurden: 1,
  coupling: 1,
  changeNovelty: 1,
  workerFit: 1,
}

const make = (partial: Partial<DispatchabilityFeatures>): DispatchabilityFeatures => ({
  ...LOW,
  ...partial,
})

test('dispatchability: a small coherent change is SAFE in 1 chunk', () => {
  const r = dispatchabilityFor(make({}))
  assert.equal(r.verdict, 'SAFE')
  assert.equal(r.chunks, 1)
  assert.ok(isDispatchable(r))
})

test('dispatchability: two proof boundaries => BOUNDED in 2 chunks', () => {
  const r = dispatchabilityFor(make({ dependencyDepth: 2 }))
  assert.equal(r.verdict, 'BOUNDED')
  assert.equal(r.chunks, 2)
})

test('dispatchability: three boundaries => HEAVY in 3 chunks', () => {
  const r = dispatchabilityFor(make({ semanticSurface: 3, dependencyDepth: 1 }))
  assert.equal(r.verdict, 'HEAVY')
  assert.equal(r.chunks, 3)
})

test('dispatchability: depth beyond 3 => NOT_DISPATCHABLE (wrong story boundary)', () => {
  const r = dispatchabilityFor(make({ dependencyDepth: 4 }))
  assert.equal(r.verdict, 'NOT_DISPATCHABLE')
  assert.equal(r.chunks, 0)
  assert.ok(!isDispatchable(r))
  assert.ok(r.reasons.join(' ').includes('dependency-depth 4'))
})

test('dispatchability: out-of-range residual risk => NOT_DISPATCHABLE (worker set up to fail)', () => {
  const r = dispatchabilityFor(make({ uncertainty: 5 }))
  assert.equal(r.verdict, 'NOT_DISPATCHABLE')
  assert.ok(r.reasons.join(' ').includes('uncertainty'))
})

test('dispatchability: anti-token-fire budgets are the hard ceiling', () => {
  assert.equal(MAX_CHUNKS_PER_STORY, 3)
  assert.equal(ANTI_TOKEN_FIRE.MAX_CHUNKS_PER_STORY, 3)
  assert.equal(ANTI_TOKEN_FIRE.MAX_SPLIT_GENERATIONS, 1)
  assert.equal(ANTI_TOKEN_FIRE.MAX_REPAIR_PER_CHUNK, 1)
  assert.equal(ANTI_TOKEN_FIRE.MAX_MODEL_ESCALATION, 1)
})
