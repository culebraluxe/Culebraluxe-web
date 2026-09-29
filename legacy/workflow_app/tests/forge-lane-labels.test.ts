import assert from 'node:assert/strict'
import test from 'node:test'
import {
  FORGE_LANE_LABEL_UNRECORDED,
  forgeLaneLabel,
  forgeSpendBlock,
} from '@/legacy/workflow_app/forge/forge-visibility'

const KNOWN: Array<[string, string]> = [
  ['scout', 'Scout'],
  ['architect', 'Architect'],
  ['lead', 'Lead'],
  ['smith', 'Smith'],
  ['inspector', 'QA'],
  ['assay', 'QA'],
  ['archive', 'Archive'],
  ['night', 'Night'],
  ['dev_ops', 'DEV_OPS'],
]

test('every run_type the engine writes maps to its human lane label', () => {
  for (const [token, label] of KNOWN) {
    assert.equal(forgeLaneLabel(token), label, token)
  }
})

test('an unknown run_type is reported verbatim, trimmed, never guessed', () => {
  assert.equal(forgeLaneLabel('banana'), 'banana')
  assert.equal(forgeLaneLabel('  banana  '), 'banana')
  assert.notEqual(forgeLaneLabel('banana'), 'QA')
})

test('an absent run_type reads as a non-empty label, never an empty string', () => {
  assert.equal(forgeLaneLabel(null), FORGE_LANE_LABEL_UNRECORDED)
  assert.equal(forgeLaneLabel(undefined), FORGE_LANE_LABEL_UNRECORDED)
  assert.equal(forgeLaneLabel(''), FORGE_LANE_LABEL_UNRECORDED)
  assert.equal(forgeLaneLabel('   '), FORGE_LANE_LABEL_UNRECORDED)
  assert.notEqual(FORGE_LANE_LABEL_UNRECORDED, '')
})

test('no input yields an empty label', () => {
  const inputs: Array<string | null | undefined> = [
    ...KNOWN.map(([token]) => token),
    'banana',
    '',
    '   ',
    null,
    undefined,
  ]
  for (const input of inputs) {
    assert.notEqual(forgeLaneLabel(input), '')
  }
})

test('forgeSpendBlock attaches the label per lane and leaves the raw lane key unchanged', () => {
  const block = forgeSpendBlock([
    { runType: 'smith', tokensInput: 1 },
    { runType: 'inspector', tokensInput: 1 },
    { runType: null, tokensInput: 1 },
  ])

  const byLane = new Map(block.lanes.map((l) => [l.lane, l]))
  assert.equal(byLane.get('smith')?.label, 'Smith')
  assert.equal(byLane.get('inspector')?.label, 'QA')
  assert.equal(byLane.get('unrecorded')?.label, FORGE_LANE_LABEL_UNRECORDED)
})
