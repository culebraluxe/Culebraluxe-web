import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { collectAssayEvidence } from '../forge/agents/assay-collect'
import { staticSliceFromGate } from '../forge/agents/exec-command'
import type { CommandResult, StaticSlice } from '../forge/agents/qa/types'
import type { RoleEffectPorts } from '../forge/agents/ports'

// ---------------------------------------------------------------------------
// FORGE-RUNTIME-WIRING-CI-01 — CI fails when a capability exists but its production supplier is
// disconnected.
//
// Astra review feature 1, and the reason it matters more than any single fix: green CI did not catch four
// High defects because it ran the scripts harness and never `workflow_app/tests`. Wiring the suite in
// (5dcab119) closed the drift class; THIS file closes the one he actually named — "a capability works in
// its focused test, but the production caller either never supplies it or drops its result" — by driving
// the REAL collector with each required input REMOVED and asserting the verdict never becomes a pass, plus
// structural guards that fail if the production wiring is deleted. No database.
// ---------------------------------------------------------------------------

const result = (command: string, passed: boolean, output: string): CommandResult => ({
  command,
  exitCode: passed ? 0 : 1,
  passed,
  excerpt: output.slice(0, 240),
  output,
})

const mapped = {
  version: 1 as const,
  hash: 'wiring-hash',
  conditions: [{ id: 'c1', text: 'the fence proves it', assertions: ['required assertion'] }],
}

const basePorts = (extra: Partial<RoleEffectPorts> = {}): RoleEffectPorts =>
  ({
    assayCommands: ['node --test the-fence.test.ts'],
    runCommand: (command: string) => result(command, true, 'ok 1 - required assertion'),
    runStatic: () => ({ archRan: true, archOk: true, archErrors: [] }) as StaticSlice,
    acceptanceMap: mapped,
    ...extra,
  }) as RoleEffectPorts

test('wiring: dropping the ACCEPTANCE MAP is not a pass', () => {
  const report = collectAssayEvidence({} as never, basePorts({ acceptanceMap: undefined }))
  assert.notEqual(report.qaPassed, true, 'an unmapped acceptance cannot be judged clean')
})

test('wiring: dropping the ASSAY COMMANDS is a FAIL, not a silent skip', () => {
  const report = collectAssayEvidence({} as never, basePorts({ assayCommands: [] }))
  assert.notEqual(report.qaPassed, true)
})

test('wiring: a FAILED MIGRATION GATE reaches the verdict through the real collector and names the rule', () => {
  const failed: StaticSlice = {
    archRan: true,
    archOk: true,
    archErrors: [],
    migrationRan: true,
    migrationOk: false,
    migrationFindings: ['prefer-bigint-over-int on column x'],
    migrationRules: ['prefer-bigint-over-int'],
  }
  const report = collectAssayEvidence({} as never, basePorts({ runStatic: () => failed }))
  assert.equal(report.qaPassed, false, 'a migration gate that failed cannot be QA PASS')
  assert.match(String(report.deliverableRejection ?? ''), /MIGRATION/i, 'the refusal names the gate')
})

test('wiring: a migration gate that COULD NOT RUN is never a pass either', () => {
  const unmeasurable: StaticSlice = {
    archRan: true,
    archOk: true,
    archErrors: [],
    migrationRan: false,
    migrationOk: false,
  }
  const report = collectAssayEvidence({} as never, basePorts({ runStatic: () => unmeasurable }))
  assert.notEqual(report.qaPassed, true, 'a required check that did not run is not clean')
})

test('wiring: dropping the NEGATIVE CONTROL records none, and one that is supplied is recorded', () => {
  const withControl = collectAssayEvidence(
    {} as never,
    basePorts({ negativeControl: { command: 'node --test control.test.ts' } }),
  )
  assert.ok(withControl.negativeControl, 'a supplied control is recorded')
  const without = collectAssayEvidence({} as never, basePorts({ negativeControl: undefined }))
  assert.equal(without.negativeControl, undefined, 'nothing is invented when none is declared')
})

test('wiring: the production runner actually supplies each of these, or CI fails here', () => {
  // The structural half. Each needle names a supplier that was once missing in production while its helper
  // was green — the acceptance map, the negative control, the observed candidate, the static gate slice —
  // plus the two seams that were dropping work (the slice adapter and the publish path).
  const runner = readFileSync(new URL('../forge/agent-runtime-role-runner.ts', import.meta.url), 'utf8')
  const mustSupply: Array<[string, string]> = [
    ['acceptanceMap ? { acceptanceMap }', 'the acceptance map'],
    ['negativeControl ? { negativeControl }', 'the negative control'],
    ['observedCandidate ? { observedCandidate }', 'the observed candidate'],
    ['runStatic: () => staticSliceForWorktree(', 'the static gate slice'],
  ]
  for (const [needle, what] of mustSupply) {
    assert.equal(runner.includes(needle), true, `production no longer supplies ${what}`)
  }

  const slice = readFileSync(new URL('../forge/agents/exec-command.ts', import.meta.url), 'utf8')
  assert.equal(slice.includes('migrationOk: result.migrationOk'), true, 'the slice adapter drops migration status')

  const publish = readFileSync(new URL('../../lib/worker-workspace/publish.ts', import.meta.url), 'utf8')
  assert.equal(publish.includes('listCommitsToPublish('), true, 'the publish path no longer scans the whole range')

  // The lane sweep lives at the publish CALLER, which is where the outcome is known.
  const publishCaller = readFileSync(new URL('../../agent-runtime/accepted-candidate-publish.ts', import.meta.url), 'utf8')
  assert.equal(publishCaller.includes('sweepLandedLaneBranches('), true, 'the publish caller no longer sweeps landed lanes')

  // And the adapter KEEPS carrying the fields it was fixed to carry, judged by behaviour not by text.
  const carried = staticSliceFromGate({
    archRan: true,
    archOk: true,
    archErrors: [],
    migrationRan: true,
    migrationOk: true,
    migrationFindings: [],
  })
  assert.equal(carried.migrationOk, true, 'staticSliceFromGate no longer carries migration status')
})
