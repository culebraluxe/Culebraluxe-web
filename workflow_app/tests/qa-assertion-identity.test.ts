import assert from 'node:assert/strict'
import test from 'node:test'

import {
  adjudicateAssay,
  adjudicateNegativeControl,
  assertionOutcome,
  runAssayCommands,
} from '../forge/agents/qa/run'
import type { AcceptanceCondition, CommandResult } from '../forge/agents/qa/types'

// ---------------------------------------------------------------------------
// ASTRA REVIEW 1.2 (2026-09-18) — ACCEPTANCE EVIDENCE CANNOT BE FORGED BY A SKIP OR A NEIGHBOUR.
//
// Both cases were reproduced against the reviewed sha and both were false PASSES:
//   `ok 1 - required assertion # SKIP missing tool`  → read as passed, so a test that never ran
//                                                      satisfied the clause
//   a DIFFERENT test whose longer name contained the   → substring matching counted it as the required
//   required name                                        assertion
// Acceptance evidence is the thing every PASS rests on, which is why a skip must read as its own SKIPPED
// state (UNPROVEN, never a pass) and identity must be exact.
// ---------------------------------------------------------------------------

test('qa-assertion: a SKIPPED assertion is skipped, never absent and never passed', () => {
  const output = [
    'ok 1 - required assertion # SKIP missing tool',
    'ok 2 - another thing',
  ].join('\n')
  assert.equal(assertionOutcome(output, 'required assertion'), 'skipped')
})

test('qa-assertion: a TODO assertion is skipped too (a placeholder ran nothing)', () => {
  assert.equal(assertionOutcome('ok 1 - required assertion # TODO later', 'required assertion'), 'skipped')
})

test('qa-assertion: a longer name that merely CONTAINS the required name does not satisfy it', () => {
  const output = 'ok 1 - the required assertion, and also three unrelated things'
  assert.equal(assertionOutcome(output, 'required assertion'), 'absent')
})

test('qa-assertion: the named assertion itself still passes, exactly as before', () => {
  assert.equal(assertionOutcome('ok 1 - required assertion', 'required assertion'), 'passed')
  assert.equal(assertionOutcome('\u2714 required assertion (1.2ms)', 'required assertion'), 'passed')
})

test('qa-assertion: a FAILED named assertion is FAIL, and failure still outranks a pass', () => {
  assert.equal(assertionOutcome('not ok 1 - required assertion', 'required assertion'), 'failed')
  const both = ['ok 1 - required assertion', 'not ok 2 - required assertion'].join('\n')
  assert.equal(assertionOutcome(both, 'required assertion'), 'failed')
})

test('qa-assertion: a skipped named assertion AFTER a real pass does not erase the pass', () => {
  const output = ['ok 1 - required assertion', 'ok 2 - required assertion # SKIP flaky'].join('\n')
  assert.equal(assertionOutcome(output, 'required assertion'), 'passed')
})

test('qa-assertion: the file-qualified ref convention still resolves by its name tail', () => {
  const output = 'ok 1 - the assertion name'
  assert.equal(assertionOutcome(output, 'workflow_app/tests/x.test.ts#the assertion name'), 'passed')
})

// ---------------------------------------------------------------------------
// FORGE-ASSERTION-SKIP-STATE-01 (2026-09-19) — A SKIP IS A NAMED STATE, NOT A SILENT ABSENCE.
//
// "Ran and passed", "ran and failed", "did not run" and "was skipped by its own proof" are four
// different facts. The reader names all four, the adjudicator names a skipped clause ASSERTION_SKIPPED
// rather than ASSERTION_NOT_RUN (never as absent), and a skipped intended assertion can never be the
// killing assertion that makes a negative control discriminate.
// ---------------------------------------------------------------------------

const FENCE_COMMAND = 'node --import tsx --test workflow_app/tests/qa-assertion-identity.test.ts'

const result = (output: string): CommandResult => ({
  command: FENCE_COMMAND,
  exitCode: 0,
  passed: true,
  excerpt: output.replace(/\s+/g, ' ').trim().slice(0, 240),
  output,
})

const mapped = (id: string, refs: string[]): AcceptanceCondition => ({
  id,
  text: `the clause ${id}`,
  assertions: refs,
})

test('qa-assertion: passed failed absent and skipped are four distinct states', () => {
  const output = [
    'ok 1 - ran and passed',
    'not ok 2 - ran and failed',
    'ok 3 - was skipped # SKIP missing tool',
  ].join('\n')
  assert.equal(assertionOutcome(output, 'ran and passed'), 'passed')
  assert.equal(assertionOutcome(output, 'ran and failed'), 'failed')
  assert.equal(assertionOutcome(output, 'was skipped'), 'skipped')
  assert.equal(assertionOutcome(output, 'never mentioned'), 'absent')
})

test('qa-assertion: a clause whose only mapped assertion is skipped is UNPROVEN and named ASSERTION_SKIPPED', () => {
  const ref = 'a mapped assertion that was skipped'
  const plan = { commands: [FENCE_COMMAND], conditions: [mapped('skip-clause', [ref])] }
  const commands = runAssayCommands(plan, () => result(`ok 1 - ${ref} # SKIP missing tool`))
  const report = adjudicateAssay({ plan, commands })
  assert.equal(report.verdict, 'UNPROVEN', 'a skipped assertion proves nothing, so the clause cannot PASS')
  assert.ok(report.unproven.includes('skip-clause'), 'the clause is named')
  assert.ok(
    report.blockers.includes(`ASSERTION_SKIPPED skip-clause ${ref}`),
    'the skip is named as its own state',
  )
  assert.ok(
    !report.blockers.includes(`ASSERTION_NOT_RUN skip-clause ${ref}`),
    'a skipped assertion is never reported as absent',
  )
  assert.deepEqual(report.missingAssertions, [], 'a skipped assertion is not a missing assertion')
})

test('qa-assertion: a SKIPPED intended assertion cannot be the killing assertion of a negative control', () => {
  const ref = 'an intended assertion the control skipped'
  const control = { command: FENCE_COMMAND, assertions: [ref] }

  const skipped = adjudicateNegativeControl({
    control,
    result: result(`ok 1 - ${ref} # SKIP missing tool`),
    conditions: [],
  })
  assert.deepEqual(skipped.outcome.killingAssertions, [], 'a skip is not a kill')
  assert.equal(skipped.survived, true, 'a control that only skipped killed nothing')

  const killed = adjudicateNegativeControl({
    control,
    result: result(`not ok 1 - ${ref}`),
    conditions: [],
  })
  assert.deepEqual(killed.outcome.killingAssertions, [ref], 'a real failure still kills')
  assert.equal(killed.survived, false)
})
