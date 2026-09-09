import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  chunkBudgetExhausted,
  chunkBudgetTotal,
  classifyChunkFailure,
  evidenceGatedPass,
  retryShapeFor,
  scopedReverifyDirective,
  triageActionFor,
} from '../forge/forge-failure-triage'

test('failure triage: each class maps to its one bounded action', () => {
  assert.equal(triageActionFor('SPEC').kind, 'rewrite-spec-retry')
  assert.equal(triageActionFor('ENV').kind, 'fix-env-retry')
  assert.equal(triageActionFor('CAPABILITY').kind, 'escalate-model')
  assert.equal(triageActionFor('VERIFICATION_GAP').kind, 'hold')
  assert.equal(triageActionFor('SCOPE_EXPANSION').kind, 'hold')
})

test('failure triage: the per-chunk budget is original + 1 repair + 1 escalation', () => {
  assert.equal(chunkBudgetTotal(), 3)
  const fresh = chunkBudgetExhausted(1, 0, 0)
  assert.equal(fresh.exhausted, false)
  assert.equal(fresh.canRepair, true)
  assert.equal(fresh.canEscalate, true)
  const afterRepair = chunkBudgetExhausted(2, 1, 0)
  assert.equal(afterRepair.canRepair, false, 'one repair used; no more same-tier')
  assert.equal(afterRepair.canEscalate, true)
  const afterEscalation = chunkBudgetExhausted(3, 1, 1)
  assert.equal(afterEscalation.exhausted, true, 'budget exhausted -> surface the unit')
})

test('failure triage: classifier follows the how-to-tell heuristic (ENV default)', () => {
  assert.equal(classifyChunkFailure({ rereadAmbiguous: true, checkFailsWithoutChange: false, scopeExpanded: false, structurallyUnverifiable: false }), 'SPEC')
  assert.equal(classifyChunkFailure({ rereadAmbiguous: false, checkFailsWithoutChange: true, scopeExpanded: false, structurallyUnverifiable: false }), 'ENV')
  assert.equal(classifyChunkFailure({ rereadAmbiguous: false, checkFailsWithoutChange: false, scopeExpanded: true, structurallyUnverifiable: false }), 'SCOPE_EXPANSION')
  assert.equal(classifyChunkFailure({ rereadAmbiguous: false, checkFailsWithoutChange: false, scopeExpanded: false, structurallyUnverifiable: true }), 'VERIFICATION_GAP')
  // Unambiguous spec + clean env whose worker can't do it => capability.
  assert.equal(classifyChunkFailure({ rereadAmbiguous: false, checkFailsWithoutChange: false, scopeExpanded: false, structurallyUnverifiable: false }), 'CAPABILITY')
})

test('failure triage: evidence-gated pass (a PASS without evidence is a FAIL)', () => {
  assert.equal(evidenceGatedPass(true, 'tests/foo.test.ts: all pass'), true)
  assert.equal(evidenceGatedPass(true, ''), false)
  assert.equal(evidenceGatedPass(true, null), false)
  assert.equal(evidenceGatedPass(false, 'some evidence'), false)
})

test('failure triage: two retry shapes (reset baseline vs incremental repair)', () => {
  // Attempt produced no verified progress -> reset to baseline (default).
  assert.equal(retryShapeFor('CAPABILITY', false), 'reset-to-baseline')
  assert.equal(retryShapeFor('SPEC', false), 'reset-to-baseline')
  // A specific verifier gap on otherwise-passing work -> incremental repair.
  assert.equal(retryShapeFor('ENV', true), 'incremental-repair')
})

test('failure triage: scoped re-verify names only open items and pins the diff', () => {
  const d = scopedReverifyDirective('abc123', ['engine.test.ts', 'migration test'])
  assert.ok(d.includes('abc123..HEAD'))
  assert.ok(d.includes('engine.test.ts; migration test'))
  assert.ok(d.includes('Do NOT re-litigate already-PASSed items'))
})
