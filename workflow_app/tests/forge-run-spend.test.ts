import assert from 'node:assert/strict'
import test from 'node:test'
import {
  FORGE_SPEND_UNRECORDED_LANE,
  forgeSpendBlock,
  type ForgeSpendRun,
} from '../forge/forge-visibility'

const run = (over: Partial<ForgeSpendRun> = {}): ForgeSpendRun => ({
  runType: 'smith',
  tokensInput: null,
  tokensOutput: null,
  costUsd: null,
  costWidgets: null,
  costSource: null,
  ...over,
})

test('per-lane spend sums the known values and totals the story', () => {
  const block = forgeSpendBlock([
    run({ runType: 'smith', tokensInput: 10, costWidgets: 2.5 }),
    run({ runType: 'smith', tokensInput: 5 }),
    run({ runType: 'lead', tokensInput: 3, costWidgets: 1.5 }),
  ])

  assert.deepEqual(
    block.lanes.map((l) => l.lane),
    ['lead', 'smith'],
  )

  const smith = block.lanes.find((l) => l.lane === 'smith')
  assert.ok(smith)
  assert.equal(smith.runs, 2)
  assert.equal(smith.spend.tokensInput, 15)
  assert.equal(smith.spend.costWidgets, 2.5)
  assert.equal(smith.spend.tokensOutput, null)

  const lead = block.lanes.find((l) => l.lane === 'lead')
  assert.ok(lead)
  assert.equal(lead.spend.tokensInput, 3)
  assert.equal(lead.spend.costWidgets, 1.5)

  assert.equal(block.total.tokensInput, 18)
  assert.equal(block.total.costWidgets, 4)
})

test('a run with no recorded spend reports null and NEVER 0', () => {
  const block = forgeSpendBlock([run()])

  assert.equal(block.total.tokensInput, null)
  assert.equal(block.total.tokensOutput, null)
  assert.equal(block.total.costUsd, null)
  assert.equal(block.total.costWidgets, null)
  assert.equal(block.total.costSource, null)
  assert.notEqual(block.total.tokensInput, 0)
  assert.notEqual(block.total.costWidgets, 0)

  assert.equal(block.lanes.length, 1)
  assert.equal(block.lanes[0].spend.tokensInput, null)
})

test('a measured zero stays zero, not null', () => {
  const block = forgeSpendBlock([run({ tokensInput: 0, tokensOutput: 0, costWidgets: 0, costUsd: 0 })])

  assert.equal(block.total.tokensInput, 0)
  assert.equal(block.total.tokensOutput, 0)
  assert.equal(block.total.costWidgets, 0)
  assert.equal(block.total.costUsd, 0)
  assert.notEqual(block.total.tokensInput, null)
  assert.notEqual(block.total.costWidgets, null)
})

test('a lane with no recorded dimension is null while the story total is the sum of what is known', () => {
  const block = forgeSpendBlock([
    run({ runType: 'smith', tokensInput: 4 }),
    run({ runType: 'qa' }),
  ])

  const qa = block.lanes.find((l) => l.lane === 'qa')
  assert.ok(qa)
  assert.equal(qa.runs, 1)
  assert.equal(qa.spend.tokensInput, null)

  assert.equal(block.total.tokensInput, 4)
})

test('the total is null when nothing is known across every lane', () => {
  const block = forgeSpendBlock([run({ runType: 'scout' }), run({ runType: 'qa' })])

  assert.equal(block.lanes.length, 2)
  assert.equal(block.total.tokensInput, null)
  assert.equal(block.total.costWidgets, null)
  assert.notEqual(block.total.costWidgets, 0)
})

test('a run whose lane is null or blank lands in an unrecorded bucket and is never dropped', () => {
  const block = forgeSpendBlock([
    run({ runType: null, tokensInput: 7 }),
    run({ runType: '   ', tokensInput: 1 }),
    run({ runType: 'smith', tokensInput: 2 }),
  ])

  assert.deepEqual(
    block.lanes.map((l) => l.lane),
    ['smith', FORGE_SPEND_UNRECORDED_LANE],
  )
  const unrecorded = block.lanes.find((l) => l.lane === FORGE_SPEND_UNRECORDED_LANE)
  assert.ok(unrecorded)
  assert.equal(unrecorded.runs, 2)
  assert.equal(unrecorded.spend.tokensInput, 8)

  assert.equal(block.total.tokensInput, 10)
})

test('cost sources are merged distinctly, and null when none is recorded', () => {
  const merged = forgeSpendBlock([
    run({ runType: 'smith', costSource: 'widgets' }),
    run({ runType: 'smith', costSource: 'widgets' }),
    run({ runType: 'smith', costSource: 'vendor' }),
  ])
  assert.equal(merged.lanes[0].spend.costSource, 'vendor+widgets')

  const none = forgeSpendBlock([run({ runType: 'smith' })])
  assert.equal(none.lanes[0].spend.costSource, null)
})

test('no runs means no lanes and an all-null total', () => {
  const block = forgeSpendBlock([])
  assert.deepEqual(block.lanes, [])
  assert.equal(block.total.tokensInput, null)
  assert.equal(block.total.costUsd, null)
  assert.equal(block.total.costSource, null)
})
