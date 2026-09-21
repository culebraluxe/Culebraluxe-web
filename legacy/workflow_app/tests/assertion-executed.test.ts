import assert from 'node:assert/strict'
import test from 'node:test'

import { adjudicateAssay, runAssayCommands } from '@/legacy/workflow_app/forge/agents/qa/run'
import type { AcceptanceCondition, CommandResult } from '@/legacy/workflow_app/forge/agents/qa/types'
import { buildArchitectDirective } from '@/legacy/workflow_app/forge/forge-architect-directive'
import { buildLeadRoutingDirective } from '@/legacy/workflow_app/forge/forge-lead-routing-prompt'

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

// ---------------------------------------------------------------------------
// ENG-FORGE-PROOF-NAMES-01 — A FILE-QUALIFIED REF RESOLVES BY ITS NAME TAIL.
//
// The mapping may name a ref the Forge way — `path#name` — but a marker line carries the NAME, never
// the path, so the reader resolves the tail after the LAST `#`. The marker rule is NOT weakened: a name
// that appears only on a non-marker line (a suite header, echoed source) is still UNPROVEN, a bare ref
// behaves exactly as before, and a failed marker naming the tail is FAIL. The accepted format is stated
// where the mapping is declared, and these tests drive every case.
// ---------------------------------------------------------------------------

const FILE = 'legacy/workflow_app/tests/assertion-executed.test.ts'

/** Adjudicate one clause with one ref against a specific recorded output. */
const adjudicateRef = (ref: string, output: string): ReturnType<typeof adjudicateAssay> => {
  const plan = { commands: [COMMAND], conditions: [mapped('clause', [ref])] }
  const results = runAssayCommands(plan, () => command(output))
  return adjudicateAssay({ plan, commands: results })
}

test('a file-qualified ref is satisfied ONLY by its own file, proven by reporter provenance', () => {
  const tail = 'a file-qualified ref is satisfied only by its own file'
  // The runner's own structured provenance: TAP's `location:` names the file the entry came from.
  const withOwnFile = [
    `\u2714 ${tail} (0.4ms)`,
    '  ---',
    `  location: '/repo/${FILE}:12:1'`,
    '  ...',
    '\u2139 tests 1',
    '\u2139 pass 1',
  ].join('\n')
  const report = adjudicateRef(`${FILE}#${tail}`, withOwnFile)
  assert.equal(report.verdict, 'PASS', "the name in the ref's OWN file satisfies the file-qualified ref")
  assert.deepEqual(report.missingAssertions, [])

  // The SAME passing line, attributed to a DIFFERENT file, does not: this is the false PASS work package B
  // closed. The qualifier is part of the claim, so it is checked rather than discarded.
  const withOtherFile = withOwnFile.replace(`location: '/repo/${FILE}`, "location: '/repo/other/x.test.ts")
  const refused = adjudicateRef(`${FILE}#${tail}`, withOtherFile)
  assert.notEqual(refused.verdict, 'PASS', 'an assertion that ran elsewhere is not evidence about this clause')
})

test('a file-qualified ref over output with NO provenance is unproven, not assumed', () => {
  // Byte-identical to the old passing fixture: a name on a marker line and nothing that says which file it ran
  // in. The old reader resolved the name tail and passed; the file was never checked because it was discarded.
  const tail = 'a file-qualified ref with no provenance'
  const report = adjudicateRef(`${FILE}#${tail}`, `\u2714 ${tail} (0.4ms)\n\u2139 tests 1`)
  assert.equal(report.verdict, 'UNPROVEN')
  assert.ok(report.unproven.includes('clause'))
  assert.ok(
    report.blockers.some((blocker) => blocker.includes('ASSERTION_ORIGIN_UNMET')),
    'the missing identity is named',
  )
})

test('a file-qualified name on a non-marker line is still unproven', () => {
  const ref = `${FILE}#a name only echoed in a header`
  const report = adjudicateRef(ref, 'a name only echoed in a header\n\u2139 tests 1')
  assert.equal(report.verdict, 'UNPROVEN', 'a non-marker line does not satisfy a ref')
  assert.deepEqual(report.missingAssertions, [{ conditionId: 'clause', assertion: ref }])
})

test('a bare ref behaves exactly as before', () => {
  const passed = adjudicateRef(PASSED, `\u2714 ${PASSED} (0.4ms)`)
  assert.equal(passed.verdict, 'PASS', 'a bare ref still matches the whole name on a marker line')
  const absent = adjudicateRef('a bare name the proof never prints', `\u2714 ${PASSED} (0.4ms)`)
  assert.equal(absent.verdict, 'UNPROVEN', 'a bare name absent from the output is still UNPROVEN')
})

test('a failed marker naming the name tail is fail not unproven, in the file it names', () => {
  const report = adjudicateRef(
    `${FILE}#a tail that failed`,
    [`\u2716 a tail that failed (0.4ms)`, '  ---', `  location: '/repo/${FILE}:40:1'`, '  ...'].join('\n'),
  )
  assert.equal(report.verdict, 'FAIL', 'a failed marker naming the tail is FAIL')
  assert.ok(report.failedConditions.includes('clause'))
  assert.ok(!report.unproven.includes('clause'), 'a ran-and-failed tail is not UNPROVEN')

  // The SAME failure attributed to another file is not this clause's failure: nothing here ran the clause's
  // assertion, so it is UNPROVEN rather than FAIL (work package B).
  const elsewhere = adjudicateRef(
    `${FILE}#a tail that failed`,
    [`\u2716 a tail that failed (0.4ms)`, '  ---', "  location: '/repo/other/x.test.ts:40:1'", '  ...'].join('\n'),
  )
  assert.notEqual(elsewhere.verdict, 'FAIL')
})

test('the accepted ref format is stated where the mapping is declared', () => {
  const architect = buildArchitectDirective('607f69a0', [COMMAND])
  const lead = buildLeadRoutingDirective({
    findings: [],
    evidenceRefs: [],
    splitEnabled: true,
    maxSmiths: 2,
    allowedProofs: [COMMAND],
  })
  const surfaces: Array<[string, string]> = [
    ['architect directive', architect],
    ['lead routing prompt', lead],
  ]
  for (const [surface, text] of surfaces) {
    assert.match(text, /path#name/, `${surface} states the file-qualified ref format`)
    assert.match(text, /marker line/, `${surface} states the marker-line rule`)
  }
})
