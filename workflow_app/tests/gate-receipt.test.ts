import assert from 'node:assert/strict'
import test from 'node:test'

import { collectAssayEvidence } from '../forge/agents/assay-collect'
import { gateChecksFor } from '../forge/agents/gate-checks'
import { buildAcceptanceMap } from '../forge/agents/qa/types'
import type { CommandResult, StaticSlice } from '../forge/agents/qa/types'
import type { RoleEffectPorts } from '../forge/agents/ports'

// ---------------------------------------------------------------------------
// FORGE-GATE-RECEIPT-01 — the run records what each check DID, and a check that did not run is never green.
//
// Astra review feature 3, with his constraint: no second verdict writer. This is a projection of checks that
// already happened, and the status vocabulary is the repository's own — "a skipped gate is visibly skipped; a
// gate that silently passes because it had no credentials is the failure mode this repository already
// recorded". The need is measured, not imagined: `pnpm test:app` prints "4 skipped" with no identity, so a
// reader cannot tell WHICH four did not run.
// ---------------------------------------------------------------------------

const result = (command: string, passed: boolean): CommandResult => ({
  command,
  exitCode: passed ? 0 : 1,
  passed,
  excerpt: passed ? 'ok 1 - required assertion' : 'not ok 1 - required assertion',
  output: passed ? 'ok 1 - required assertion' : 'not ok 1 - required assertion',
})

const VALID_MAP = buildAcceptanceMap({
  acceptance: ['it works'],
  // THE REAL BUILDER, THE REAL HASH (work package F). A hand-written `hash: 'h'` is not a map hash: the
  // adjudicator compares it against the mapping it was given, so the "valid" fixture was refused with
  // ACCEPTANCE_MAP_CHANGED and every `qaPassed` expectation built on it was meaningless.
  assertions: { 'it works': ['required assertion'] },
})

const slice = (extra: Partial<StaticSlice> = {}): StaticSlice => ({
  archRan: true,
  archOk: true,
  archErrors: [],
  ...extra,
})

test('gate receipt: every frozen proof is recorded with its own identity and status', () => {
  const checks = gateChecksFor({
    commands: ['node --test a.test.ts', 'node --test b.test.ts'],
    results: [result('node --test a.test.ts', true), result('node --test b.test.ts', false)],
    staticGate: slice(),
    acceptanceMapped: true,
    negativeControl: null,
  })
  assert.deepEqual(
    checks.filter((c) => c.id.startsWith('proof:')).map((c) => [c.id, c.status]),
    [
      ['proof:node --test a.test.ts', 'passed'],
      ['proof:node --test b.test.ts', 'failed'],
    ],
  )
})

test('gate receipt: a proof that produced no result is UNAVAILABLE, never passed', () => {
  const checks = gateChecksFor({
    commands: ['node --test missing.test.ts'],
    results: [],
    staticGate: slice(),
    acceptanceMapped: true,
    negativeControl: null,
  })
  const proof = checks.find((c) => c.id.startsWith('proof:'))
  assert.equal(proof?.status, 'unavailable')
})

test('gate receipt: a SKIPPED architecture gate is skipped, not passed — the whole point of the story', () => {
  const checks = gateChecksFor({
    commands: [],
    results: [],
    staticGate: { archRan: false, archOk: false, archErrors: [] },
    acceptanceMapped: true,
    negativeControl: null,
  })
  assert.equal(checks.find((c) => c.id === 'architecture')?.status, 'skipped')
})

test('gate receipt: the migration half distinguishes passed, failed, skipped and NOT CONFIGURED', () => {
  const withGate = (extra: Partial<StaticSlice>) =>
    gateChecksFor({ commands: [], results: [], staticGate: slice(extra), acceptanceMapped: true, negativeControl: null })
      .find((c) => c.id === 'migration')

  assert.equal(withGate({ migrationRan: true, migrationOk: true })?.status, 'passed')
  assert.equal(
    withGate({ migrationRan: true, migrationOk: false, migrationFindings: ['prefer-bigint-over-int'] })?.status,
    'failed',
  )
  assert.equal(withGate({ migrationRan: false, migrationOk: false })?.status, 'skipped')
  assert.equal(withGate({})?.status, 'not-configured', 'a slice with no migration opinion is not a pass')
})

test('gate receipt: an unmapped acceptance and an undeclared control are NOT CONFIGURED, with reasons', () => {
  const checks = gateChecksFor({
    commands: [],
    results: [],
    staticGate: slice(),
    acceptanceMapped: false,
    negativeControl: null,
  })
  const mapping = checks.find((c) => c.id === 'acceptance-map')
  const control = checks.find((c) => c.id === 'negative-control')
  assert.equal(mapping?.status, 'not-configured')
  assert.ok(mapping?.reason)
  assert.equal(control?.status, 'not-configured')
  assert.ok(control?.reason)
})

// ---------------------------------------------------------------------------
// WORK PACKAGE E — THE RECEIPT AGREES WITH WHAT WAS OBSERVED.
//
// Two reproductions from the re-review:
//   1. the control RAN and killed nothing → QA UNPROVEN, receipt `passed`, reason "killed nothing";
//   2. a missing runner returned a refusal with NO receipt at all.
// A receipt that disagrees with the verdict is worse than no receipt: it is the row a reader trusts when the
// verdict is surprising. The projection stays a projection — it reads the adjudicator's own outcomes and never
// recomputes QA policy.
// ---------------------------------------------------------------------------

test('receipt: a control that RAN and killed nothing is not `passed`', () => {
  const surviving = { command: 'node --test x.test.ts', ran: true, unmeasurable: false, killingAssertions: [] }
  const checks = gateChecksFor({
    commands: [],
    results: [],
    staticGate: slice(),
    acceptanceMapped: true,
    negativeControl: surviving,
    controlSurvived: true,
  })
  const control = checks.find((c) => c.id === 'negative-control')
  assert.equal(control?.status, 'failed', 'a surviving control is the reason the verdict is UNPROVEN')
  assert.match(String(control?.reason), /killed nothing/)

  const killing = { command: 'node --test x.test.ts', ran: true, unmeasurable: false, killingAssertions: ['k'] }
  const passed = gateChecksFor({
    commands: [],
    results: [],
    staticGate: slice(),
    acceptanceMapped: true,
    negativeControl: killing,
    controlSurvived: false,
  }).find((c) => c.id === 'negative-control')
  assert.equal(passed?.status, 'passed')
})

test('receipt: a proof that could not be EXECUTED is unavailable, not an ordinary failed assertion', () => {
  const unmeasurable: CommandResult = {
    command: 'node --test x.test.ts',
    exitCode: 1,
    passed: false,
    excerpt: 'spawn timeout after 900s',
    output: '',
    unmeasurable: true,
  }
  const checks = gateChecksFor({
    commands: ['node --test x.test.ts'],
    results: [unmeasurable],
    staticGate: slice(),
    acceptanceMapped: true,
    negativeControl: null,
  })
  const proof = checks.find((c) => c.id.startsWith('proof:'))
  assert.equal(proof?.status, 'unavailable')
  assert.match(String(proof?.reason), /timeout/)
})

test('receipt: the missing-runner refusal INCLUDES the receipt, and the pass path really passes', () => {
  const withControl = {
    assayCommands: ['node --test a.test.ts'],
    runStatic: () => slice(),
    acceptanceMap: VALID_MAP,
    negativeControl: { command: 'node --test control.test.ts' },
  } as Partial<RoleEffectPorts>

  // NO RUNNER: a refusal, with every proof reported unavailable for the same reason.
  const refused = collectAssayEvidence({} as never, withControl as RoleEffectPorts)
  assert.equal(refused.qaPassed, false)
  assert.ok(Array.isArray(refused.gateChecks) && refused.gateChecks.length > 0, 'the refusal carries a receipt')
  assert.equal(
    refused.gateChecks?.find((c) => c.id.startsWith('proof:'))?.status,
    'unavailable',
    'nothing could be executed, and the receipt says so',
  )

  // THE PASS PATH: a working runner and a control that kills something.
  const passing = collectAssayEvidence({} as never, {
    ...withControl,
    runCommand: (command: string) =>
      command.includes('control')
        ? {
            command,
            exitCode: 1,
            passed: false,
            excerpt: 'not ok 1 - required assertion',
            output: 'not ok 1 - required assertion',
          }
        : {
            command,
            exitCode: 0,
            passed: true,
            excerpt: 'ok 1 - required assertion',
            output: 'ok 1 - required assertion',
          },
  } as RoleEffectPorts)
  assert.equal(passing.qaPassed, true, 'a genuine success case, asserted — not inferred')
  assert.equal(passing.gateChecks?.find((c) => c.id === 'negative-control')?.status, 'passed')
  assert.equal(passing.gateChecks?.find((c) => c.id.startsWith('proof:'))?.status, 'passed')
})

test('receipt: the refusal path carries the acceptance-map receipt, where a reader most needs it', () => {
  const ports = (extra: Partial<RoleEffectPorts> = {}): RoleEffectPorts =>
    ({
      assayCommands: ['node --test a.test.ts'],
      runCommand: (command: string) => result(command, true),
      runStatic: () => slice(),
      acceptanceMap: VALID_MAP,
      ...extra,
    }) as RoleEffectPorts

  const refusing = collectAssayEvidence({} as never, ports({ acceptanceMap: undefined }))
  assert.equal(refusing.qaPassed, false)
  assert.equal(
    refusing.gateChecks?.find((c) => c.id === 'acceptance-map')?.status,
    'not-configured',
    'the refusal path is where a reader most needs the receipt',
  )
})
