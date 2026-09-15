import assert from 'node:assert/strict'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { commandRunner } from '../forge/agents/exec-command'
import { collectAssayEvidence } from '../forge/agents/assay-collect'
import type { RoleEffectPorts } from '../forge/agents/ports'
import { adjudicateAssay, runAssayCommands } from '../forge/agents/qa/run'
import type { CommandResult } from '../forge/agents/qa/types'

// ---------------------------------------------------------------------------
// THE QA SEAM — the base set.
//
// These are the tests that were missing, and their absence is why a working
// proof came back as a failure: every existing test checked the adjudicator with
// hand-made `passed` booleans, so nothing exercised "run the command, then judge
// what came back". Three facts live at that seam and each one has a test here:
//
//   1. a command that RUNS and exits 0 is a PASS            (1+1=2)
//   2. one that RUNS and exits non-zero is a FAIL
//   3. one that could not run at all is a GAP, never a FAIL
//
// Add to this file as the seam grows; keep the assertions about outcomes, not
// about mechanics.
// ---------------------------------------------------------------------------

const scratch = () => mkdtempSync(join(tmpdir(), 'forge-qa-seam-'))

test('a command that runs and exits 0 is a PASS (1+1=2)', () => {
  const dir = scratch()
  try {
    const command = 'node -e "process.exit(0)"'
    const run = commandRunner(dir)
    const results = runAssayCommands({ candidateSha: 'sha-1', commands: [command] }, run)
    const report = adjudicateAssay({ plan: { candidateSha: 'sha-1', commands: [command] }, commands: results })
    assert.equal(results[0]?.passed, true)
    assert.equal(results[0]?.unmeasurable, undefined)
    assert.equal(report.verdict, 'PASS')
    assert.deepEqual(report.blockers, [])
    assert.equal(report.verifiedSha, 'sha-1')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('a command that runs and exits non-zero is a FAIL, named CMD_FAIL', () => {
  const dir = scratch()
  try {
    const command = 'node -e "process.exit(3)"'
    const run = commandRunner(dir)
    const results = runAssayCommands({ candidateSha: 'sha-1', commands: [command] }, run)
    const report = adjudicateAssay({ plan: { candidateSha: 'sha-1', commands: [command] }, commands: results })
    assert.equal(results[0]?.passed, false)
    assert.equal(results[0]?.exitCode, 3)
    assert.equal(results[0]?.unmeasurable, undefined)
    assert.equal(report.verdict, 'FAIL')
    assert.deepEqual(report.blockers, [`CMD_FAIL ${command}`])
    assert.equal(report.verifiedSha, null)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('a command that CANNOT RUN is a GAP (INCOMPLETE), never CMD_FAIL', () => {
  // The live bug: a spawn that never starts reports status null, and coercing that to exit 1 recorded
  // CMD_FAIL for a proof that passes 11/11 when it can actually be run. A directory that does not exist
  // is the honest way to reproduce "could not run" without inventing a fake runner.
  const missing = join(tmpdir(), 'forge-qa-seam-does-not-exist-9f2c')
  const command = 'node --import tsx --test workflow_app/tests/forge-doctor-report.test.ts'
  const run = commandRunner(missing)
  const results = runAssayCommands({ candidateSha: 'sha-1', commands: [command] }, run)
  const report = adjudicateAssay({ plan: { candidateSha: 'sha-1', commands: [command] }, commands: results })

  assert.equal(results[0]?.unmeasurable, true, 'a command that never started is unmeasurable')
  assert.equal(results[0]?.exitCode, -1, 'and its exit code is NOT 1, which would read as a test failure')
  assert.equal(report.verdict, 'INCOMPLETE')
  assert.deepEqual(report.blockers, [`CMD_UNMEASURABLE ${command}`])
  assert.ok(!report.blockers.some((b) => b.startsWith('CMD_FAIL')), 'a gap must never be reported as a failure')
  assert.equal(report.verifiedSha, null)
})

test('the gap and the failure are distinguishable in the same run', () => {
  const dir = scratch()
  try {
    const failing = 'node -e "process.exit(1)"'
    const run = commandRunner(dir)
    const results: CommandResult[] = [
      ...runAssayCommands({ candidateSha: 'sha-1', commands: ['node -e "process.exit(0)"'] }, run),
      ...runAssayCommands({ candidateSha: 'sha-1', commands: [failing] }, run),
      ...runAssayCommands({ candidateSha: 'sha-1', commands: ['echo hi'] }, commandRunner(join(tmpdir(), 'nope-9f2c'))),
    ]
    const report = adjudicateAssay({
      plan: { candidateSha: 'sha-1', commands: ['node -e "process.exit(0)"', failing, 'echo hi'] },
      commands: results,
    })
    assert.equal(report.verdict, 'INCOMPLETE', 'a gap outranks a failure: we cannot certify either way')
    assert.ok(report.blockers.includes(`CMD_FAIL ${failing}`))
    assert.ok(report.blockers.includes('CMD_UNMEASURABLE echo hi'))
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('the QA collector reports a gap as a gap and never as a failed command', () => {
  const ports = {
    assayCommands: ['echo not-runnable-here'],
    runCommand: commandRunner(join(tmpdir(), 'forge-qa-seam-nope-9f2c')),
  } as unknown as RoleEffectPorts

  const evidence = collectAssayEvidence({ candidateSha: 'sha-1' } as never, ports)
  assert.equal(evidence.qaPassed, false)
  assert.equal(evidence.verificationGap, true, 'a gap is a verification gap')
  assert.equal(
    (evidence as { failedCommands?: string[] }).failedCommands,
    undefined,
    'nothing failed — nothing was measured, and repair must not be sent after it',
  )
})
