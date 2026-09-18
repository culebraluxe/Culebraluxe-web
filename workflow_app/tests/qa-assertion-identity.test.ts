import assert from 'node:assert/strict'
import test from 'node:test'

import { assertionOutcome } from '../forge/agents/qa/run'

// ---------------------------------------------------------------------------
// ASTRA REVIEW 1.2 (2026-09-18) — ACCEPTANCE EVIDENCE CANNOT BE FORGED BY A SKIP OR A NEIGHBOUR.
//
// Both cases were reproduced against the reviewed sha and both were false PASSES:
//   `ok 1 - required assertion # SKIP missing tool`  → read as passed, so a test that never ran
//                                                      satisfied the clause
//   a DIFFERENT test whose longer name contained the   → substring matching counted it as the required
//   required name                                        assertion
// Acceptance evidence is the thing every PASS rests on, which is why a skip must read as ABSENT
// (UNPROVEN, the honest verdict for evidence that does not exist) and identity must be exact.
// ---------------------------------------------------------------------------

test('qa-assertion: a SKIPPED assertion is absent, never passed', () => {
  const output = [
    'ok 1 - required assertion # SKIP missing tool',
    'ok 2 - another thing',
  ].join('\n')
  assert.equal(assertionOutcome(output, 'required assertion'), 'absent')
})

test('qa-assertion: a TODO assertion is absent too (a placeholder ran nothing)', () => {
  assert.equal(assertionOutcome('ok 1 - required assertion # TODO later', 'required assertion'), 'absent')
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
