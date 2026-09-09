import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  DEFAULT_RUN_WALL_CLOCK_MS,
  runWallClockBudgetMs,
} from './agent-runtime-adapter'
import {
  buildGroundingDirective,
  buildRtkCompressionDirective,
  buildRunGuardrailsDirective,
  buildRunPassDirective,
  buildSmithWorkDecompositionDirective,
  parseSmithPlan,
  parseRunPassStop,
  resolveRtkBin,
  rtkAvailable,
  runPassBudget,
  smithPlanExceedsBounds,
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

test('rtk: resolved only when the binary exists; RTK_BIN override wins', () => {
  const env = (x: Record<string, string>): NodeJS.ProcessEnv => x as unknown as NodeJS.ProcessEnv
  assert.equal(resolveRtkBin(env({ PATH: '/usr/bin:/bin:/nonexistent' })), null, 'no rtk on that PATH')
  // Override must be honored even when PATH cannot satisfy it.
  assert.equal(resolveRtkBin(env({ PATH: '/usr/bin', RTK_BIN: '/opt/tools/rtk' })), '/opt/tools/rtk')
  assert.equal(rtkAvailable(env({ PATH: '/usr/bin', RTK_BIN: '/opt/tools/rtk' })), true)
  assert.equal(rtkAvailable(env({ PATH: '/usr/bin' })), false)
})

test('rtk: the compression directive appears only when rtk is available', () => {
  const env = (x: Record<string, string>): NodeJS.ProcessEnv => x as unknown as NodeJS.ProcessEnv
  assert.equal(buildRtkCompressionDirective(env({ PATH: '/usr/bin' })), null, 'no directive without rtk')
  const d = buildRtkCompressionDirective(env({ PATH: '/usr/bin', RTK_BIN: '/opt/tools/rtk' }))
  assert.ok(d?.includes('context compressor'))
  assert.ok(d?.includes('rtk test'))
  assert.ok(d?.includes('Never dump a large raw command transcript'))
})

test('smith decomposition: directive encodes size-by-boundaries and <=3 serial chunks', () => {
  const d = buildSmithWorkDecompositionDirective()
  assert.ok(d.includes('COUPLING'))
  assert.ok(d.includes('NEVER by file count or LOC alone'))
  assert.ok(d.includes('SMITH_PLAN:'))
  assert.ok(d.includes('<=3 coherent chunks'))
  assert.ok(d.includes('STOP and HOLD'))
})

test('smith decomposition: SMITH_PLAN parses for the data lake; oversize is detectable', () => {
  const medium = parseSmithPlan('planning\nSMITH_PLAN: {"size":"MEDIUM","chunks":2,"proofs":["persistence test","workflow test"]}\ndone')
  assert.equal(medium?.size, 'MEDIUM')
  assert.equal(medium?.chunks, 2)
  assert.deepEqual(medium?.proofs, ['persistence test', 'workflow test'])
  assert.equal(smithPlanExceedsBounds(medium), false)
  const bad = parseSmithPlan('SMITH_PLAN: {"size":"OVERSIZED","chunks":5}')
  assert.equal(smithPlanExceedsBounds(bad), true)
  const noPlan = parseSmithPlan('no structured plan here')
  assert.equal(noPlan, null)
  assert.equal(smithPlanExceedsBounds(noPlan), false)
})
