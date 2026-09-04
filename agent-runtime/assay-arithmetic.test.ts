import assert from 'node:assert/strict'
import test from 'node:test'

import {
  isArithmeticAssayPass,
  parseAssayArithmeticFacts,
} from './assay-arithmetic'
import { isCleanAssayEvidence } from './candidate-assay-handoff'

test('V5-10 regression: 26/26, fail 0, exit 0 stays PASS even with bogus failed-commands prose', () => {
  const summary =
    'base/candidate SHA 0ad01a917f2fc069d4ae7c634116ed31fe561f8f — command → exit 0, tail: tests 26, pass 26, fail 0, cancelled 0, skipped 0 | failed commands: `pnpm exec tsx --test agent-runtime/**/*feed*.test.ts agent-runtime/**/*work*.test.ts`'

  const facts = parseAssayArithmeticFacts(summary)
  assert.deepEqual(facts.exitCodes, [0])
  assert.equal(facts.testsTotal, 26)
  assert.equal(facts.testsPassed, 26)
  assert.equal(facts.testsFailed, 0)
  assert.equal(facts.policyViolations, 0)
  assert.equal(isArithmeticAssayPass(facts), true)
  assert.equal(
    isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary: summary }),
    true,
  )
})

test('V5-11 regression: 25/25, 0 fail, exit 0 stays PASS even with bogus failed-commands prose', () => {
  const summary =
    'pnpm exec tsx --test agent-runtime/**/*team*.test.ts agent-runtime/**/*routing*.test.ts → exit 0, 25/25 pass, 0 fail | failed commands: `same successful command`'

  const facts = parseAssayArithmeticFacts(summary)
  assert.deepEqual(facts.exitCodes, [0])
  assert.equal(facts.testsTotal, 25)
  assert.equal(facts.testsPassed, 25)
  assert.equal(facts.testsFailed, 0)
  assert.equal(isArithmeticAssayPass(facts), true)
  assert.equal(
    isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary: summary }),
    true,
  )
})

test('non-zero exit is a mathematical failure', () => {
  const summary = 'command → exit 1, tests 26, pass 25, fail 1'
  const facts = parseAssayArithmeticFacts(summary)
  assert.equal(isArithmeticAssayPass(facts), false)
  assert.equal(
    isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary: summary }),
    false,
  )
})

test('positive failed count is a mathematical failure even if prose says success', () => {
  const summary = 'everything wonderful; exit 0; tests 26; pass 25; fail 1'
  assert.equal(
    isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary: summary }),
    false,
  )
})

test('passed must equal total when both counters are present', () => {
  assert.equal(
    isCleanAssayEvidence({
      resultStatus: 'Complete',
      testsSummary: 'exit 0; tests 26; pass 25; fail 0',
    }),
    false,
  )
})

test('explicit runtime policy violation defeats otherwise clean arithmetic', () => {
  assert.equal(
    isCleanAssayEvidence({
      resultStatus: 'Complete',
      testsSummary: 'exit 0; tests 26; pass 26; fail 0 | TEST-MODE VIOLATION (SCOPED): pnpm test',
    }),
    false,
  )
})

test('Hold can never become a pass just because its arithmetic is clean', () => {
  assert.equal(
    isCleanAssayEvidence({
      resultStatus: 'Hold',
      testsSummary: 'exit 0; tests 26; pass 26; fail 0',
    }),
    false,
  )
})

test('legacy no-number evidence still uses fail-closed vocabulary', () => {
  assert.equal(
    isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary: 'all checks passed' }),
    true,
  )
  assert.equal(
    isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary: 'required file missing' }),
    false,
  )
})
