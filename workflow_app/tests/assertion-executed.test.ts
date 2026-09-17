import assert from 'node:assert/strict'
import test from 'node:test'

import { adjudicateAssay, runAssayCommands } from '../forge/agents/qa/run'
import type { AcceptanceCondition, CommandResult } from '../forge/agents/qa/types'

// ---------------------------------------------------------------------------
// ENG-FORGE-ASSERTION-RAN-01 — A REFERENCED ASSERTION MUST HAVE RUN.
//
// A mapping that NAMES an assertion proves nothing by itself. Before this, a clause was satisfied by
// `assertions.length > 0`, so a name the proof never executed read as PROVEN. These lock the rule at the
// adjudicator seam: an assertion that does not appear in the EXECUTED proof output is UNPROVEN and both the
// clause and the missing assertion are named; an assertion that ran and passed satisfies the clause; and an
// assertion that ran and FAILED is FAIL — never UNPROVEN, because the proof did speak about it.
//
// ONE RECORDED OUTPUT DRIVES ALL THREE CASES, so an outcome can never be explained by a different fixture.
// ---------------------------------------------------------------------------

const COMMAND = 'node --import tsx --test workflow_app/tests/assertion-executed.test.ts'

const PASSED = 'a mapped assertion that ran and passed satisfies its clause'
const FAILED = 'a mapped assertion that ran and failed is FAIL not UNPROVEN'
const NEVER_RAN = 'an assertion the proof never mentions'

/** One recorded proof output: PASSED ran and passed, FAILED ran and failed, NEVER_RAN is absent. */
const RECORDED = [
  `\u2714 ${PASSED} (0.4ms)`,
  `\u2716 ${FAILED} (0.4ms)`,
  '\u2139 tests 2',
  '\u2139 pass 1',
  '\u2139 fail 1',
].join('\n')

const command = (output: string): CommandResult => ({
  command: COMMAND,
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

/** Adjudicate one plan against the single recorded output. */
const adjudicate = (conditions: AcceptanceCondition[]): ReturnType<typeof adjudicateAssay> => {
  const plan = { commands: [COMMAND], conditions }
  const results = runAssayCommands(plan, () => command(RECORDED))
  return adjudicateAssay({ plan, commands: results })
}

test('a mapped assertion the proof never ran is UNPROVEN and names the missing assertion', () => {
  const report = adjudicate([mapped('never-ran', [NEVER_RAN])])
  assert.equal(report.verdict, 'UNPROVEN', 'a named-but-unrun assertion proves nothing')
  assert.ok(report.unproven.includes('never-ran'), 'the clause is named')
  assert.ok(report.blockers.includes('UNPROVEN never-ran'), 'the clause is named on the blockers')
  assert.ok(
    report.blockers.includes(`ASSERTION_NOT_RUN never-ran ${NEVER_RAN}`),
    'the missing assertion is named on the blockers',
  )
  assert.deepEqual(report.missingAssertions, [{ conditionId: 'never-ran', assertion: NEVER_RAN }])
})

test('a mapped assertion that ran and passed satisfies its clause', () => {
  const report = adjudicate([mapped('ran-passed', [PASSED])])
  assert.equal(report.verdict, 'PASS', 'an assertion that ran and passed satisfies the clause')
  assert.deepEqual(report.unproven, [])
  assert.deepEqual(report.failedConditions, [])
  assert.deepEqual(report.missingAssertions, [])
  assert.deepEqual(report.blockers, [])
})

test('a mapped assertion that ran and failed is FAIL not UNPROVEN', () => {
  const report = adjudicate([mapped('ran-failed', [FAILED])])
  assert.equal(report.verdict, 'FAIL', 'a failed assertion is a FAIL, never UNPROVEN')
  assert.ok(report.failedConditions.includes('ran-failed'), 'the clause is named as failed')
  assert.ok(report.blockers.includes(`ASSERTION_FAILED ran-failed ${FAILED}`))
  assert.ok(!report.unproven.includes('ran-failed'), 'a ran-and-failed assertion is not UNPROVEN')
})

test('one recorded proof output drives absent passed and failed', () => {
  const report = adjudicate([
    mapped('absent', [NEVER_RAN]),
    mapped('passed', [PASSED]),
    mapped('failed', [FAILED]),
  ])
  assert.equal(report.verdict, 'FAIL', 'the failed assertion outranks the others')
  assert.deepEqual(report.unproven, ['absent'], 'only the absent clause is UNPROVEN')
  assert.deepEqual(report.failedConditions, ['failed'])
  assert.deepEqual(report.missingAssertions, [{ conditionId: 'absent', assertion: NEVER_RAN }])
})

test('a clause with no mapped assertion is still UNPROVEN', () => {
  const report = adjudicate([{ id: 'unmapped', text: 'the unmapped clause', assertions: [] }])
  assert.equal(report.verdict, 'UNPROVEN')
  assert.ok(report.blockers.includes('UNPROVEN unmapped'))
})
