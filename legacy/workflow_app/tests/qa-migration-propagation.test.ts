import assert from 'node:assert/strict'
import test from 'node:test'

import { staticSliceFromGate } from '@/legacy/workflow_app/forge/agents/exec-command'
import { adjudicateAssay } from '@/legacy/workflow_app/forge/agents/qa/run'
import type { CommandResult } from '@/legacy/workflow_app/forge/agents/qa/types'
import type { StaticGateResult } from '@/legacy/workflow_app/forge/forge-static-gate'

// ---------------------------------------------------------------------------
// FORGE-QA-MIGRATION-PROPAGATION-01 — A FAILED MIGRATION GATE REACHES THE QA VERDICT.
//
// The static gate reports the migration hard gate (squawk) on its own result, but the adapter that
// narrows it to the QA slice used to copy only arch/semgrep/knip, so an unsafe migration was invisible
// to the collector and judged PASS. Each case drives a gate fixture through `staticSliceFromGate` AND
// `adjudicateAssay` — the adapter plus the collector — never the adapter alone.
// ---------------------------------------------------------------------------

const COMMAND = 'node --import tsx --test workflow_app/tests/qa-migration-propagation.test.ts'
const RULE = 'require-concurrent-index-creation'

const gate = (over: Partial<StaticGateResult>): StaticGateResult => ({
  workspace: '/tmp/candidate',
  roots: ['workflow_app'],
  archRan: true,
  archOk: true,
  archErrors: [],
  semgrepRan: true,
  semgrepFindings: [],
  knipRan: true,
  knipFindings: [],
  knipCounts: {},
  knipGroupCount: 0,
  migrationRan: true,
  migrationOk: true,
  migrationFindings: [],
  migrationRules: [],
  ok: true,
  ...over,
})

const passedCommand = (): CommandResult => ({
  command: COMMAND,
  exitCode: 0,
  passed: true,
  excerpt: '',
  output: 'ok 1 - a passing proof',
})

/** Adjudicate one gate result through the adapter and then the collector. */
const collect = (result: StaticGateResult) =>
  adjudicateAssay({
    plan: { commands: [COMMAND] },
    commands: [passedCommand()],
    staticGate: staticSliceFromGate(result),
  })

test('unsafe migration produces FAIL through the collector naming the rule', () => {
  const report = collect(
    gate({
      migrationOk: false,
      migrationFindings: [
        `${RULE} db/migrations/999_unsafe.sql:1 — CREATE INDEX without CONCURRENTLY`,
      ],
      migrationRules: [RULE],
    }),
  )
  assert.equal(report.verdict, 'FAIL', 'an unsafe migration must refuse the candidate')
  assert.ok(
    report.blockers.some((blocker) => blocker.includes(RULE)),
    'the failing squawk rule is named in the blockers',
  )
})

test('a required migration check that could not run is never a pass', () => {
  const report = collect(
    gate({
      migrationRan: false,
      migrationOk: false,
      migrationFindings: [
        'migration lint unavailable: squawk is not installed (npm install -g squawk-cli)',
      ],
      migrationRules: [],
    }),
  )
  assert.notEqual(report.verdict, 'PASS', 'a check that could not run is not a pass')
  assert.ok(
    report.blockers.some((blocker) => blocker.startsWith('MIGRATION_UNMEASURABLE')),
    'the unmeasurable migration check is named',
  )
})

test('a safe migration passes through the collector', () => {
  const report = collect(gate({ migrationRan: true, migrationOk: true }))
  assert.equal(report.verdict, 'PASS', 'a clean migration check leaves the verdict alone')
})

test('the fence drives all three cases through the collector', () => {
  const unsafe = collect(
    gate({
      migrationOk: false,
      migrationFindings: [`${RULE} db/migrations/999_unsafe.sql:1`],
      migrationRules: [RULE],
    }),
  )
  const unmeasurable = collect(
    gate({
      migrationRan: false,
      migrationOk: false,
      migrationFindings: ['migration lint unavailable: squawk is not installed'],
    }),
  )
  const safe = collect(gate({ migrationRan: true, migrationOk: true }))

  assert.equal(unsafe.verdict, 'FAIL', 'unsafe is FAIL')
  assert.ok(
    unsafe.blockers.some((blocker) => blocker.includes(RULE)),
    'unsafe names the rule',
  )
  assert.notEqual(unmeasurable.verdict, 'PASS', 'could-not-run is never a pass')
  assert.equal(safe.verdict, 'PASS', 'safe is PASS')
})
