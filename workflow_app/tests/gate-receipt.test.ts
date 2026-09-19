import assert from 'node:assert/strict'
import test from 'node:test'

import { collectAssayEvidence } from '../forge/agents/assay-collect'
import { gateChecksFor } from '../forge/agents/gate-checks'
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

test('gate receipt: the collector ATTACHES the receipt on both the pass and the refusal path', () => {
  const ports = (extra: Partial<RoleEffectPorts> = {}): RoleEffectPorts =>
    ({
      assayCommands: ['node --test a.test.ts'],
      runCommand: (command: string) => result(command, true),
      runStatic: () => slice(),
      acceptanceMap: {
        version: 1 as const,
        hash: 'h',
        conditions: [{ id: 'c1', text: 'it works', assertions: ['required assertion'] }],
      },
      ...extra,
    }) as RoleEffectPorts

  const passing = collectAssayEvidence({} as never, ports())
  assert.ok(Array.isArray(passing.gateChecks) && passing.gateChecks.length > 0, 'no receipt on the pass path')

  const refusing = collectAssayEvidence({} as never, ports({ acceptanceMap: undefined }))
  assert.equal(refusing.qaPassed, false)
  assert.equal(
    refusing.gateChecks?.find((c) => c.id === 'acceptance-map')?.status,
    'not-configured',
    'the refusal path is where a reader most needs the receipt',
  )
})
