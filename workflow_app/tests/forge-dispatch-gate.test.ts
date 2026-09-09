import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { SmithChunk, SmithExecutionPlan } from '../forge/forge-execution-shaping'
import type { DispatchabilityFeatures } from '../forge/forge-dispatchability'
import { assessSmithDispatch } from '../forge/forge-dispatch-gate'

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

test('dispatch gate: a sound bounded 2-chunk plan with proofs dispatches (GO)', () => {
  const a = assessSmithDispatch(plan([chunk(1), chunk(2, { dependsOn: [1] })]), { qualitativeFeatures: LOW })
  assert.equal(a.structuralOk, true)
  assert.equal(a.verdict, 'GO')
})

test('dispatch gate: a missing plan or 4-chunk plan HOLDs on structural rules', () => {
  assert.equal(assessSmithDispatch(null).verdict, 'HOLD')
  const a = assessSmithDispatch(plan([chunk(1), chunk(2), chunk(3), chunk(4)]))
  assert.equal(a.verdict, 'HOLD')
  assert.equal(a.structuralOk, false)
  assert.ok(a.reasons.join(' ').includes('4th chunk is HOLD'))
})

test('dispatch gate: qualitative NOT_DISPATCHABLE (the 6-surface monster) HOLDs', () => {
  const a = assessSmithDispatch(
    plan([chunk(1), chunk(2), chunk(3)]),
    { qualitativeFeatures: { ...LOW, semanticSurface: 6, dependencyDepth: 4 } },
  )
  assert.equal(a.verdict, 'HOLD')
  assert.ok(a.qualitative?.verdict === 'NOT_DISPATCHABLE')
})

test('dispatch gate: a plan with no runnable proof at all is FLAG (weak proof)', () => {
  const a = assessSmithDispatch(plan([
    chunk(1, { proof: 'verify by inspection' }),
    chunk(2, { proof: 'manually confirm behavior' }),
  ]))
  assert.equal(a.structuralOk, true)
  assert.equal(a.verdict, 'FLAG', 'no machine-checkable acceptance must flag, not sail clean as GO')
  assert.ok(a.difficulty?.gate === 'flag')
})
