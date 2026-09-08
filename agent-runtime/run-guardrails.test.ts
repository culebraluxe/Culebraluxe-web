import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  DEFAULT_RUN_WALL_CLOCK_MS,
  runWallClockBudgetMs,
} from './agent-runtime-adapter'
import { buildRunGuardrailsDirective } from './run-guardrails'

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
