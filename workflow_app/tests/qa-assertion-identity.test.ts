import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { promisify } from 'node:util'

import {
  adjudicateAssay,
  adjudicateNegativeControl,
  assertionEntries,
  assertionOutcome,
  assertionResolution,
  runAssayCommands,
} from '../forge/agents/qa/run'
import type { AcceptanceCondition, CommandResult } from '../forge/agents/qa/types'

const execFileAsync = promisify(execFile)

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

// ---------------------------------------------------------------------------
// WORK PACKAGE B — ASSERTION ORIGIN.
//
// Reproduced against the reviewed sha: acceptance `a.test.ts#same assertion`, executed command
// `node --test b.test.ts`, output `ok 1 - same assertion`, verdict QA PASS. The file qualifier was discarded
// before matching, so an assertion in one file satisfied acceptance assigned to another.
//
// THE PROVENANCE HERE IS REAL REPORTER OUTPUT, not handwritten markers: these fences RUN `node --test` with the
// JUnit reporter over temporary fixtures created and destroyed inside the invocation. JUnit is the built-in
// reporter that names the FILE for a PASSING test — measured on node 24, TAP carries `location:` only inside a
// failure's YAML block, which is why a TAP-only pass cannot establish origin and is refused.
// ---------------------------------------------------------------------------

const runJunit = async (files: Record<string, string>): Promise<string> => {
  const dir = await mkdtemp(join(tmpdir(), 'forge-assertion-origin-'))
  try {
    for (const [name, contents] of Object.entries(files)) await writeFile(join(dir, name), contents)
    const { stdout } = await execFileAsync(
      process.execPath,
      ['--test', '--test-reporter=junit', ...Object.keys(files)],
      {
        cwd: dir,
        encoding: 'utf8',
        // THE CHILD MUST NOT INHERIT THIS FILE'S TEST CONTEXT. `node:test` refuses to run test files recursively
        // ("run() is being called recursively within a test file. skipping running files.") and then produces NO
        // output at all — a silent empty capture that made every assertion in this file look unmatchable.
        env: { ...process.env, NODE_TEST_CONTEXT: undefined },
      },
    ).catch((error: { stdout?: string }) => {
      // A FAILING FIXTURE IS A VALID CAPTURE. `node --test` exits non-zero when a test fails, and the reporter's
      // XML is still on stdout — a fence that needs a failing test to prove "failure is still a failure" would
      // otherwise have to pretend the run succeeded.
      if (typeof error.stdout === 'string' && error.stdout.length > 0) return { stdout: error.stdout, stderr: '' }
      throw error
    })
    return stdout
  } finally {
    // Destroyed inside the invocation: no fixture outlives the test that made it.
    await rm(dir, { recursive: true, force: true })
  }
}

const PASSING = "import test from 'node:test'\nimport assert from 'node:assert/strict'\n"
const sameAssertion = (body = 'assert.equal(1, 1)') => `${PASSING}test('same assertion', () => { ${body} })\n`

test('origin: a named assertion in the RIGHT file satisfies acceptance (real junit provenance)', async () => {
  const output = await runJunit({ 'a.test.ts': sameAssertion(), 'b.test.ts': sameAssertion() })
  assert.equal(assertionOutcome(output, 'a.test.ts#same assertion'), 'passed')
  assert.equal(assertionOutcome(output, 'b.test.ts#same assertion'), 'passed')
})

test('origin: a passing b.test.ts assertion CANNOT satisfy a.test.ts#same assertion', async () => {
  // The exact reproduction: only b runs, and the acceptance names a.
  const output = await runJunit({ 'b.test.ts': sameAssertion() })
  const resolution = assertionResolution(output, 'a.test.ts#same assertion')
  assert.equal(resolution.status, 'absent')
  assert.equal(resolution.status === 'absent' && resolution.reason, 'wrong-file')
  assert.match(resolution.status === 'absent' ? resolution.detail : '', /b\.test\.ts/)
})

test('origin: the same name in two files is distinguishable in ONE command', async () => {
  const output = await runJunit({ 'a.test.ts': sameAssertion(), 'b.test.ts': sameAssertion() })
  // Two entries, same name, different files — the case that made "flatten into a bag of names" wrong.
  const entries = assertionEntries(output).filter((entry) => entry.name === 'same assertion')
  assert.equal(entries.length, 2)
  assert.equal(new Set(entries.map((entry) => entry.file ?? '')).size, 2)
  // A file that did not run is still refused, with no arbitrary pick between the two.
  assert.equal(assertionOutcome(output, 'c.test.ts#same assertion'), 'absent')
})

test('origin: a file-qualified ref over output with NO provenance is refused, not assumed', async () => {
  // TAP (node's default for a captured stream) names the assertion and not the file. "We cannot say which file
  // this came from" is not the same claim as "it ran in the right file".
  const tap = ['TAP version 13', '# Subtest: same assertion', 'ok 1 - same assertion', '1..1'].join('\n')
  const resolution = assertionResolution(tap, 'a.test.ts#same assertion')
  assert.equal(resolution.status, 'absent')
  assert.equal(resolution.status === 'absent' && resolution.reason, 'no-provenance')
  // ...and the same output still satisfies an UNQUALIFIED ref, exactly as before.
  assert.equal(assertionOutcome(tap, 'same assertion'), 'passed')
})

test('origin: an unqualified ref is refused when the name is ambiguous across files', async () => {
  const output = await runJunit({ 'a.test.ts': sameAssertion(), 'b.test.ts': sameAssertion() })
  const resolution = assertionResolution(output, 'same assertion')
  assert.equal(resolution.status, 'absent', 'two origins is a choice, and this code does not choose')
  assert.equal(resolution.status === 'absent' && resolution.reason, 'ambiguous')
})

test('origin: a FAILED assertion in the right file is still a failure, and skips are still nonpassing', async () => {
  const failing = await runJunit({ 'a.test.ts': sameAssertion('assert.equal(1, 2)') })
  assert.equal(assertionOutcome(failing, 'a.test.ts#same assertion'), 'failed')

  const skipped = await runJunit({
    'a.test.ts': `${PASSING}test('same assertion', { skip: true }, () => {})\n`,
  })
  assert.equal(assertionOutcome(skipped, 'a.test.ts#same assertion'), 'skipped')
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

test('origin: the ADJUDICATOR refuses the wrong-file case and names it', () => {
  // Provenance here is TAP's own `location:` field, in the YAML diagnostics block a marker line carries — the
  // exact shape the runner emits for a failing entry. The acceptance names a.test.ts; the assertion ran in b.
  const output = [
    'TAP version 13',
    'not ok 1 - same assertion',
    '  ---',
    "  location: '/repo/workflow_app/tests/b.test.ts:3:1'",
    '  ...',
    '1..1',
  ].join('\n')
  const report = adjudicateAssay({
    plan: {
      conditions: [mapped('clause-1', ['a.test.ts#same assertion'])],
      commands: [FENCE_COMMAND],
    },
    commands: [result(output)],
    staticGate: { archRan: true, archOk: true, archErrors: [] },
  })
  assert.equal(report.verdict, 'UNPROVEN', 'not FAIL: the intended assertion never ran here')
  const blockers = report.blockers.join(' | ')
  assert.match(blockers, /UNPROVEN clause-1/)
  assert.match(blockers, /ASSERTION_ORIGIN_UNMET clause-1 a\.test\.ts#same assertion/)
})

test('origin: a negative control cannot claim a kill from the wrong file', async () => {
  const control = { command: 'node --test a.test.ts', assertions: ['a.test.ts#same assertion'] }
  const conditions = [mapped('clause-1', ['a.test.ts#same assertion'])]

  // The control's output shows the intended name FAILING — in b.test.ts. A kill from the wrong file is not a kill.
  const wrongFile = await runJunit({ 'b.test.ts': sameAssertion('assert.equal(1, 2)') })
  const wrongAdjudicated = adjudicateNegativeControl({
    control,
    result: result(wrongFile),
    conditions,
  })
  assert.equal(wrongAdjudicated.outcome.ran, true)
  assert.deepEqual(wrongAdjudicated.outcome.killingAssertions, [])
  assert.equal(wrongAdjudicated.survived, true)

  // ...and a genuine kill in the right file still counts.
  const rightFile = await runJunit({ 'a.test.ts': sameAssertion('assert.equal(1, 2)') })
  const rightAdjudicated = adjudicateNegativeControl({
    control,
    result: result(rightFile),
    conditions,
  })
  assert.deepEqual(rightAdjudicated.outcome.killingAssertions, ['a.test.ts#same assertion'])
  assert.equal(rightAdjudicated.survived, false)
})
