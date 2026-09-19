import assert from 'node:assert/strict'
import test from 'node:test'

import { collectAssayEvidence } from '../forge/agents/assay-collect'
import { negativeControlForStory } from '../forge/agents/negative-control-supplier'
import { buildAcceptanceMap } from '../forge/agents/qa/types'
import type { RoleEffectPorts } from '../forge/agents/ports'

// ---------------------------------------------------------------------------
// FORGE-NEGATIVE-CONTROL-WIRING-01 — the control reaches QA through production composition.
//
// Astra review 1.4, measured: the collector and the adjudicator both support a negative control, the test
// supplied one by hand, and the production `rolePorts` never did — so no ordinary run could reach the
// capability, which is why "a fence proves it can fail" was a promise no dispatch could keep. Two halves
// are asserted here: the SUPPLIER (what the runner now reads from the story row) and the COLLECTOR's
// behaviour when that supplier hands it something, or nothing.
// ---------------------------------------------------------------------------

const portsWith = (extra: Partial<RoleEffectPorts>): RoleEffectPorts =>
  ({
    assayCommands: ['node --test the-fence.test.ts'],
    runCommand: (command: string) => result(command, true, 'ok 1 - required assertion'),
    ...extra,
  }) as RoleEffectPorts

test('negative control supply: a story that declares a control supplies it, and one that does not supplies NOTHING', () => {
  assert.deepEqual(negativeControlForStory({ negativeControlCommand: 'node --test control.test.ts' }), {
    command: 'node --test control.test.ts',
  })
  // Absent stays absent: never `{}`, which would look like a control that ran and killed nothing.
  assert.equal(negativeControlForStory({ negativeControlCommand: null }), undefined)
  assert.equal(negativeControlForStory({}), undefined)
  assert.equal(negativeControlForStory(null), undefined)
  // A blank command is nothing, and pretending to run it would be worse than saying we did not.
  assert.equal(negativeControlForStory({ negativeControlCommand: '   ' }), undefined)
})

test('negative control supply: the assertions list is deliberately omitted, so the mapping stays the one source', () => {
  const control = negativeControlForStory({ negativeControlCommand: 'node --test control.test.ts' })
  assert.ok(control)
  assert.equal('assertions' in control, false, 'the plan acceptance mapping already names what is intended')
})

const result = (command: string, passed: boolean, output: string) => ({
  command,
  exitCode: passed ? 0 : 1,
  passed,
  excerpt: output.slice(0, 240),
  output,
})

// BUILT BY THE REAL BUILDER (work package F): a hand-written hash is refused as ACCEPTANCE_MAP_CHANGED, which
// would make every negative control assertion below pass for a reason that has nothing to do with the control.
const mapped = buildAcceptanceMap({
  acceptance: ['the fence proves it'],
  assertions: { 'the fence proves it': ['required assertion'] },
})

test('negative control collector: a supplied control that kills an intended assertion is recorded as such', () => {
  const report = collectAssayEvidence(
    {} as never,
    portsWith({
      negativeControl: { command: 'node --test control.test.ts' },
      acceptanceMap: mapped,
      // The control RUNS THE SAME FENCE with the behaviour inverted, so its assertion reports failed.
      runCommand: (command: string) =>
        command.includes('control.test.ts')
          ? result(command, false, 'not ok 1 - required assertion')
          : result(command, true, 'ok 1 - required assertion'),
    }),
  )
  assert.equal(report.negativeControl?.ran, true)
  assert.deepEqual(report.negativeControl?.killingAssertions, ['required assertion'])
})

test('negative control collector: no control supplied means no control recorded — absent is not a pass', () => {
  const report = collectAssayEvidence(
    {} as never,
    portsWith({ acceptanceMap: mapped }),
  )
  assert.equal(report.negativeControl, undefined, 'nothing is invented for a story that declared nothing')
})
