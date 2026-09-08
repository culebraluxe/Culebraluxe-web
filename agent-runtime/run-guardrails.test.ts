import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  DEFAULT_RUN_WALL_CLOCK_MS,
  runWallClockBudgetMs,
} from './agent-runtime-adapter'
import {
  buildGroundingDirective,
  buildRunGuardrailsDirective,
  buildRunPassDirective,
  parseRunPassStop,
  runPassBudget,
} from './run-guardrails'

test('run budget: env override wins; missing falls back to the default', () => {
  assert.equal(runWallClockBudgetMs({ ...process.env, FORGE_RUN_MAX_MS: '1200000' }), 1_200_000)
  assert.equal(runWallClockBudgetMs({ ...process.env, FORGE_RUN_MAX_MS: '' }), DEFAULT_RUN_WALL_CLOCK_MS)
  assert.equal(runWallClockBudgetMs(process.env), DEFAULT_RUN_WALL_CLOCK_MS)
})

test('guardrails: every role directive carries cost lines + a hard budget + stop-to-HOLD', () => {
  const d = buildRunGuardrailsDirective({ ...process.env, FORGE_RUN_MAX_MS: (45 * 60_000).toString() })
  assert.ok(d.includes('Relative model cost'))
  assert.ok(d.includes('HARD 45-minute wall-clock budget'))
  assert.ok(/STOP|do NOT grind/i.test(d))
})

test('bounded autonomy: one pass then stop by default; more rope via FORGE_RUN_PASSES', () => {
  assert.equal(runPassBudget(process.env), 1)
  assert.equal(runPassBudget({ ...process.env, FORGE_RUN_PASSES: '4' }), 4)
  const d = buildRunPassDirective({ ...process.env, FORGE_RUN_PASSES: '1' })
  assert.ok(d.includes('Bounded autonomy: execute at most 1 pass'))
  assert.ok(d.includes('FORGE_PASS_STOP: COMPLETE | NEEDS_REVIEW | NEEDS_MORE_PASSES'))
})

test('bounded autonomy: a run stop disposition is machine-parseable', () => {
  assert.equal(parseRunPassStop('Some prose\nFORGE_PASS_STOP: NEEDS_REVIEW'), 'NEEDS_REVIEW')
  assert.equal(parseRunPassStop('FORGE_PASS_STOP: COMPLETE'), 'COMPLETE')
  assert.equal(parseRunPassStop('no disposition here'), null)
})

test('grounding: judgment roles must answer from context, never repo turn-search', () => {
  const d = buildGroundingDirective()
  assert.ok(d.includes('JUDGMENT role'))
  assert.ok(d.includes('Do NOT run repo scans, broad searches, or exploratory tool turns'))
  assert.ok(d.includes('HOLD/STOP'))
})
