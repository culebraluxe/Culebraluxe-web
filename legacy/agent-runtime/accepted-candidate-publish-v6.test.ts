import assert from 'node:assert/strict'
import test from 'node:test'

import { publishAcceptedCandidateAfterAssay } from './accepted-candidate-publish'
import type { AssayEvidence } from './assay-evidence'

const CANDIDATE = 'd'.repeat(40)

function evidence(overrides: Partial<AssayEvidence> = {}): AssayEvidence {
  return {
    version: 1,
    candidateSha: CANDIDATE,
    verifiedSha: CANDIDATE,
    requiredCommands: ['verify'],
    commandResults: [{
      command: 'verify',
      exitCode: 0,
      signal: null,
      timedOut: false,
      durationMs: 1,
      tests: { total: 26, passed: 26, failed: 0 },
      stdoutTail: 'failed commands words are presentation only',
      stderrTail: '',
    }],
    policyViolations: [],
    startedAt: '2026-09-04T00:00:00.000Z',
    endedAt: '2026-09-04T00:00:01.000Z',
    verdict: 'PASS',
    failureCode: null,
    failureDetail: null,
    ...overrides,
  }
}

test('structured PASS + Complete + exact candidate reaches safe publish callback despite hostile prose', async () => {
  let published = false
  const report = await publishAcceptedCandidateAfterAssay({
    role: 'verifier',
    resultStatus: 'Complete',
    testsSummary: 'failed commands policy failure words must not override structured evidence',
    assayEvidence: evidence(),
    candidateCommit: CANDIDATE,
    publish: async ({ candidateCommit }) => {
      published = true
      assert.equal(candidateCommit, CANDIDATE)
      return {
        outcome: 'published',
        candidateCommit,
        publishedMainHash: candidateCommit,
      }
    },
  })

  assert.equal(published, true)
  assert.equal(report.action, 'published')
})

test('structured FAIL never reaches publish even if prose claims success', async () => {
  let published = false
  const report = await publishAcceptedCandidateAfterAssay({
    role: 'verifier',
    resultStatus: 'Complete',
    testsSummary: '26/26 pass everything perfect',
    assayEvidence: evidence({
      verdict: 'FAIL',
      failureCode: 'ASSAY_TEST_FAILED',
      failureDetail: 'exit 1',
    }),
    candidateCommit: CANDIDATE,
    publish: async () => {
      published = true
      throw new Error('publish must not be called')
    },
  })

  assert.equal(published, false)
  assert.equal(report.action, 'not-eligible')
})

test('structured PASS still requires terminal run status Complete', async () => {
  let published = false
  const report = await publishAcceptedCandidateAfterAssay({
    role: 'verifier',
    resultStatus: 'Hold',
    assayEvidence: evidence(),
    candidateCommit: CANDIDATE,
    publish: async () => {
      published = true
      throw new Error('publish must not be called')
    },
  })
  assert.equal(published, false)
  assert.equal(report.action, 'not-eligible')
})

test('structured PASS cannot publish a different candidate SHA', async () => {
  let published = false
  const report = await publishAcceptedCandidateAfterAssay({
    role: 'verifier',
    resultStatus: 'Complete',
    assayEvidence: evidence(),
    candidateCommit: 'e'.repeat(40),
    publish: async () => {
      published = true
      throw new Error('publish must not be called')
    },
  })
  assert.equal(published, false)
  assert.equal(report.action, 'not-eligible')
})
