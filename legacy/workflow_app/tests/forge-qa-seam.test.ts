import assert from 'node:assert/strict'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { commandRunner } from '@/legacy/workflow_app/forge/agents/exec-command'
import { collectAssayEvidence } from '@/legacy/workflow_app/forge/agents/assay-collect'
import type { RoleEffectPorts } from '@/legacy/workflow_app/forge/agents/ports'
import { adjudicateAssay, runAssayCommands } from '@/legacy/workflow_app/forge/agents/qa/run'
import type { CommandResult } from '@/legacy/workflow_app/forge/agents/qa/types'
import { forgeEvidenceFromAgentResult } from '@/legacy/workflow_app/forge/forge-role-mapping'
import { routeQaResult } from '@/legacy/workflow_app/forge/qa-repair-policy'

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
    const results = runAssayCommands({ commands: [command] }, run)
    const report = adjudicateAssay({ plan: { commands: [command] }, commands: results })
    assert.equal(results[0]?.passed, true)
    assert.equal(results[0]?.unmeasurable, undefined)
    assert.equal(report.verdict, 'PASS')
    assert.deepEqual(report.blockers, [])
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('a command that runs and exits non-zero is a FAIL, named CMD_FAIL', () => {
  const dir = scratch()
  try {
    const command = 'node -e "process.exit(3)"'
    const run = commandRunner(dir)
    const results = runAssayCommands({ commands: [command] }, run)
    const report = adjudicateAssay({ plan: { commands: [command] }, commands: results })
    assert.equal(results[0]?.passed, false)
    assert.equal(results[0]?.exitCode, 3)
    assert.equal(results[0]?.unmeasurable, undefined)
    assert.equal(report.verdict, 'FAIL')
    assert.deepEqual(report.blockers, [`CMD_FAIL ${command}`])
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('a command that CANNOT RUN did NOT pass, and is named as unmeasurable', () => {
  // The live bug this guarded: a spawn that never starts reports status null, and coercing that to exit 1
  // recorded CMD_FAIL for a proof that passes 11/11 when it can actually be run. A directory that does not
  // exist is the honest way to reproduce "could not run" without inventing a fake runner.
  const missing = join(tmpdir(), 'forge-qa-seam-does-not-exist-9f2c')
  const command = 'node --import tsx --test workflow_app/tests/forge-doctor-report.test.ts'
  const run = commandRunner(missing)
  const results = runAssayCommands({ commands: [command] }, run)
  const report = adjudicateAssay({ plan: { commands: [command] }, commands: results })

  assert.equal(results[0]?.unmeasurable, true, 'a command that never started is unmeasurable')
  assert.equal(results[0]?.exitCode, -1, 'and its exit code is NOT 1, which would read as a test failure')
  // DID THE TESTS PASS? No — a command that never ran did not pass. The record still says WHY, per command,
  // so "could not run" never masquerades as "ran and failed".
  assert.equal(report.verdict, 'FAIL')
  assert.deepEqual(report.blockers, [`CMD_UNMEASURABLE ${command}`])
  assert.ok(!report.blockers.some((b) => b.startsWith('CMD_FAIL')), 'a gap must never be reported as a failure')
})

test('a failed command and a command that could not run are named separately in one run', () => {
  const dir = scratch()
  try {
    const failing = 'node -e "process.exit(1)"'
    const run = commandRunner(dir)
    const results: CommandResult[] = [
      ...runAssayCommands({ commands: ['node -e "process.exit(0)"'] }, run),
      ...runAssayCommands({ commands: [failing] }, run),
      ...runAssayCommands({ commands: ['echo hi'] }, commandRunner(join(tmpdir(), 'nope-9f2c'))),
    ]
    const report = adjudicateAssay({
      plan: { commands: ['node -e "process.exit(0)"', failing, 'echo hi'] },
      commands: results,
    })
    assert.equal(report.verdict, 'FAIL')
    assert.ok(report.blockers.includes(`CMD_FAIL ${failing}`))
    assert.ok(report.blockers.includes('CMD_UNMEASURABLE echo hi'))
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('the QA collector FAILS when there is nothing to test', () => {
  // QA is reached only after a
  // Smith produced work, on a story that carries its own frozen proofs, so no commands means something
  // upstream is broken — and a broken upstream is NAMED. It is not passed (that is how an empty plan shipped
  // green) and it is not held behind a gap (that hides why the chain stopped).
  const ports = {
    assayCommands: [],
    runCommand: commandRunner(join(tmpdir(), 'forge-qa-seam-nope-9f2c')),
  } as unknown as RoleEffectPorts

  const evidence = collectAssayEvidence({} as never, ports)
  assert.equal(evidence.qaPassed, false, 'nothing to test is a failure')
  assert.match(
    (evidence as { deliverableRejection?: string }).deliverableRejection ?? '',
    /NO_ASSAY_COMMANDS/,
    'the failure names what is missing',
  )
})

// ---------------------------------------------------------------------------
// ENG-QA-SINGLE-VERDICT-01 — ONE ADJUDICATOR.
//
// The QA verdict must have exactly one author: `adjudicateAssay`. The mapping is a
// projector, the collector applies the adjudicator's PASS|FAIL|INCOMPLETE, and the
// router HOLDs on an INCOMPLETE gap instead of sending repair after untested code.
// ---------------------------------------------------------------------------

const SHA = 'a'.repeat(40)

test('the QA mapping is a projector: no verdict is authored outside adjudicateAssay', () => {
  // The mapping used to read `result.assayEvidence.verdict`/`verifiedSha` and write
  // qaPassed/qaVerifiedSha/failureClass — a second QA-verdict author. It now projects
  // only the candidate plus the durable gap, so `collectAssayEvidence` (fed by
  // `adjudicateAssay`) is the sole writer. Asserting on the projection output keys
  // makes a reintroduced second computation fail here.
  const mapped = forgeEvidenceFromAgentResult({
    nodeId: 'qa_verify',
    result: {
      resultStatus: 'Complete',
      completion: 100,
      notes: '',
      testsSummary: null,
      commitHash: null,
      runtimeAdapter: 'tunit',
      modelProfile: 'tunit',
      externalRunId: 'run-1',
      startedAt: new Date(0).toISOString(),
      endedAt: new Date(1).toISOString(),
      assayEvidence: {
        version: 1,
        verdict: 'PASS',
        failureCode: null,
        failureDetail: null,
        candidateSha: SHA,
        verifiedSha: SHA,
        requiredCommands: ['node --test'],
        commandResults: [],
        policyViolations: [],
        startedAt: new Date(0).toISOString(),
        endedAt: new Date(1).toISOString(),
      },
    },
    current: { candidateSha: SHA },
  })
  assert.equal(mapped.qaPassed, undefined, 'the projector must not author a verdict')
  assert.equal(mapped.qaVerifiedSha, undefined, 'the projector must not certify a SHA')
  assert.equal(mapped.failureClass, undefined, 'a failure the tests did not cause is never CODE_DEFECT')
  assert.equal(
    mapped.candidateSha,
    undefined,
    'and it must not hand QA a git identity — QA has no relationship to git',
  )
})

test('a command that could not run fails the story, and the router stops instead of repairing', () => {
  // The old shape of this test proved that an INCOMPLETE gap reached the router as a gap and HOLDed. The gap
  // vocabulary is gone (the verdict is did-the-tests-pass), so what protects the chain now is the captain's
  // own rule: a QA failure carries no disposition, and `routeQaResult` HOLDs without one. Untested code is
  // still never sent to repair — by a simpler mechanism.
  const ports = {
    assayCommands: ['echo not-runnable-here'],
    runCommand: commandRunner(join(tmpdir(), 'forge-qa-seam-router-nope-9f2c')),
  } as unknown as RoleEffectPorts

  const evidence = collectAssayEvidence({} as never, ports)
  assert.equal(evidence.qaPassed, false, 'a command that never ran is not a pass')
  assert.equal(
    (evidence as { failureClass?: string }).failureClass,
    undefined,
    'a failure the tests did not cause is never labelled CODE_DEFECT',
  )

  const route = routeQaResult({
    verdict: 'FAIL',
    disposition: (evidence as { disposition?: never }).disposition,
    state: { repairAttempts: 0, replanAttempts: 0 },
  })
  assert.equal(route.action, 'hold', 'no disposition means no repair, on the first failure')
})

// ---------------------------------------------------------------------------
// THE CAPTAIN'S RULE (2026-09-16): "if the QA fails first time there should not
// be a retry unless the chain fixes something, otherwise it is wasting compute."
//
// The enforcement is structural, not a budget: the deterministic Assay decides
// PASS or not-pass and NOTHING ELSE — it authors no REPAIR/REPLAN disposition —
// and `routeQaResult` fails closed into HOLD without one. So a first QA failure
// can never dispatch a repair. Only a lane that CLASSIFIES the failure (a model,
// which this path does not run) could author a disposition, and that is the
// "unless the chain fixes something" case.
// ---------------------------------------------------------------------------

test('a deterministic QA FAIL authors no disposition, so the router can only HOLD — never repair', () => {
  const dir = scratch()
  try {
    const realFailure = collectAssayEvidence({ candidateSha: SHA } as never, {
      assayCommands: ['node -e "process.exit(3)"'],
      runCommand: commandRunner(dir),
    } as unknown as RoleEffectPorts)

    assert.equal(realFailure.qaPassed, false, 'a command that ran and failed is NOT a pass')
    assert.equal(
      (realFailure as { disposition?: string }).disposition,
      undefined,
      'the Assay authors no repair disposition — it is a test runner, not a classifier',
    )

    const route = routeQaResult({
      verdict: 'FAIL',
      disposition: (realFailure as { disposition?: never }).disposition,
      state: { repairAttempts: 0, replanAttempts: 0 },
    })
    assert.equal(route.action, 'hold', 'no disposition means no retry, on the first failure')
    assert.equal(
      route.action === 'hold' ? route.reason?.includes('disposition') : false,
      true,
      'and the HOLD says why, so the operator is not left guessing',
    )
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

