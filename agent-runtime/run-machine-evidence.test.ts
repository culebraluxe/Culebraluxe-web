import assert from 'node:assert/strict'
import test from 'node:test'

import { isCleanRunMachineEvidence } from '../lib/forge-run-evidence'
import { finalizeAssayEvidence } from './assay-evidence'
import { runMachineEvidenceFromFinish } from './run-machine-evidence'

const BASE = 'a'.repeat(40)
const CANDIDATE = 'b'.repeat(40)

test('Smith and other model lanes project numeric facts into the common Run shape', () => {
  const evidence = runMachineEvidenceFromFinish({
    role: 'builder',
    resultStatus: 'Complete',
    notes: `Smith complete.\n\nExecution workspace: branch=agent/story/run worktree=/tmp/run base=main@${BASE}`,
    testsSummary: 'opencode exit code 0 | Tests: 26/26 passed; failed: 0',
  })

  assert.equal(evidence.baseCommitHash, BASE)
  assert.equal(evidence.commandsTotal, 1)
  assert.equal(evidence.commandsPassed, 1)
  assert.equal(evidence.commandsFailed, 0)
  assert.equal(evidence.testsTotal, 26)
  assert.equal(evidence.testsPassed, 26)
  assert.equal(evidence.testsFailed, 0)
  assert.equal(evidence.policyViolationCount, 0)
  assert.equal(evidence.failureCode, null)
  assert.match(evidence.evidenceDetail ?? '', /Smith complete/)
  assert.equal(isCleanRunMachineEvidence(evidence), true)
})

test('deterministic Assay projects rich evidence into the same generic Run fields', () => {
  const assay = finalizeAssayEvidence({
    version: 1,
    candidateSha: CANDIDATE,
    verifiedSha: CANDIDATE,
    requiredCommands: ['pnpm test:a', 'pnpm test:b'],
    commandResults: [
      {
        command: 'pnpm test:a',
        exitCode: 0,
        signal: null,
        timedOut: false,
        durationMs: 10,
        tests: { total: 10, passed: 10, failed: 0 },
        stdoutTail: '10/10 passed',
        stderrTail: '',
      },
      {
        command: 'pnpm test:b',
        exitCode: 0,
        signal: null,
        timedOut: false,
        durationMs: 20,
        tests: { total: 16, passed: 16, failed: 0 },
        stdoutTail: '16/16 passed',
        stderrTail: '',
      },
    ],
    policyViolations: [],
    startedAt: '2026-09-04T00:00:00.000Z',
    endedAt: '2026-09-04T00:00:01.000Z',
  })

  const evidence = runMachineEvidenceFromFinish({
    role: 'verifier',
    resultStatus: 'Complete',
    notes: 'Assay finished.',
    testsSummary: 'human prose is not authoritative',
    assayEvidence: assay,
  })

  assert.equal(evidence.baseCommitHash, CANDIDATE)
  assert.equal(evidence.commandsTotal, 2)
  assert.equal(evidence.commandsPassed, 2)
  assert.equal(evidence.commandsFailed, 0)
  assert.equal(evidence.testsTotal, 26)
  assert.equal(evidence.testsPassed, 26)
  assert.equal(evidence.testsFailed, 0)
  assert.equal(evidence.policyViolationCount, 0)
  assert.equal(evidence.failureCode, null)
  assert.match(evidence.evidenceDetail ?? '', /Assay verdict: PASS/)
  assert.equal(isCleanRunMachineEvidence(evidence), true)
})

test('Assay failure uses the common failure and counter fields', () => {
  const assay = finalizeAssayEvidence({
    version: 1,
    candidateSha: CANDIDATE,
    verifiedSha: CANDIDATE,
    requiredCommands: ['pnpm test:a'],
    commandResults: [
      {
        command: 'pnpm test:a',
        exitCode: 1,
        signal: null,
        timedOut: false,
        durationMs: 10,
        tests: { total: 10, passed: 9, failed: 1 },
        stdoutTail: '9/10 passed',
        stderrTail: 'one failed',
      },
    ],
    policyViolations: [],
    startedAt: '2026-09-04T00:00:00.000Z',
    endedAt: '2026-09-04T00:00:01.000Z',
  })

  const evidence = runMachineEvidenceFromFinish({
    role: 'verifier',
    resultStatus: 'Hold',
    notes: 'Assay failed.',
    testsSummary: null,
    assayEvidence: assay,
  })

  assert.equal(evidence.commandsFailed, 1)
  assert.equal(evidence.testsFailed, 1)
  assert.equal(evidence.failureCode, 'ASSAY_TEST_FAILED')
  assert.equal(isCleanRunMachineEvidence(evidence), false)
})
