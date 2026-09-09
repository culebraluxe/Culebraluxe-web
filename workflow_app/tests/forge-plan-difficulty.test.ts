import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { SmithChunk, SmithExecutionPlan } from '../forge/forge-execution-shaping'
import {
  chunkHasRunnableProof,
  difficultyFeaturesForPlan,
  scorePlan,
} from '../forge/forge-plan-difficulty'
import { FLAG_BELOW } from '../forge/forge-difficulty-scorer'

const chunk = (id: number, over: Partial<SmithChunk> = {}): SmithChunk => ({
  id,
  outcome: `outcome ${id}`,
  surface: [`path${id}/a.ts`],
  invariant: `invariant ${id}`,
  proof: `pnpm exec tsx --test path${id}/a.test.ts`,
  ...over,
})

const plan = (chunks: SmithChunk[], size: SmithExecutionPlan['size'] = 'MEDIUM'): SmithExecutionPlan => ({
  size,
  chunks,
})

test('plan difficulty: runnable proof detection requires a real command token', () => {
  assert.equal(chunkHasRunnableProof('pnpm exec tsx --test x.test.ts'), true)
  assert.equal(chunkHasRunnableProof('run the targeted test'), false, 'prose is not a signal')
})

test('plan difficulty: a bounded 3-chunk plan with proofs is dispatchable', () => {
  const p = plan([chunk(1), chunk(2, { dependsOn: [1] }), chunk(3, { dependsOn: [2] })])
  const { features, pSuccess, gate } = scorePlan(p)
  assert.equal(features.filesTouched, 3)
  assert.equal(features.depDepth, 3)
  assert.equal(features.hasAcceptance, true)
  assert.equal(features.genericType, false)
  assert.ok(pSuccess >= FLAG_BELOW, `expected dispatch, got p=${pSuccess.toFixed(3)} gate=${gate}`)
  assert.equal(gate, 'dispatch')
})

test('plan difficulty: a chunk without a runnable proof reads as unshaped/generic', () => {
  const p = plan([chunk(1, { proof: 'verify it works by inspection' }), chunk(2)])
  const features = difficultyFeaturesForPlan(p)
  assert.equal(features.genericType, true, 'a chunk with only prose proof is not shaped for a worker')
})
