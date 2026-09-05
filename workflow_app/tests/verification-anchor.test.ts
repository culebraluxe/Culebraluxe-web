import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  compileVerificationContext,
  evaluateVerification,
  type AnchorEvidence,
} from '../forge/verification-anchor'

// ENG-FORGE-HARDEN-08 — independent verification context + authoritative
// anchors. QA verifies the artifact, not Smith's explanation of the artifact.

const ctx = () =>
  compileVerificationContext({
    storyId: 'S1',
    candidateSha: 'abc',
    acceptanceCriteria: ['AC1'],
    testScope: ['src'],
    requiredAnchors: ['test', 'git'],
  })

const testEv = (exitCode: number, sha = 'abc'): AnchorEvidence => ({
  kind: 'test', source: 'system', exitCode, verifiedSha: sha,
})
const gitEv = (sha = 'abc'): AnchorEvidence => ({ kind: 'git', source: 'system', verifiedSha: sha })

test('QA context is compiled independently and excludes Smith narrative', () => {
  const c = ctx()
  assert.equal(c.smithNarrativeExcluded, true)
  assert.equal(c.candidateSha, 'abc')
  assert.deepEqual(c.acceptanceCriteria, ['AC1'])
})

test('Smith claims PASS but the test anchor FAILS -> QA FAIL', () => {
  const smithClaim: AnchorEvidence = { kind: 'test', source: 'agent', exitCode: 0, detail: 'tests pass' }
  const r = evaluateVerification({ requiredAnchors: ['test'], evidence: [smithClaim, testEv(1)], candidateSha: 'abc' })
  assert.equal(r.passed, false)
  assert.ok(r.blockers.some((b) => b.includes('test anchor failed')))
})

test('agent testimony cannot satisfy an anchor requirement (no system anchor)', () => {
  const claim: AnchorEvidence = { kind: 'test', source: 'agent', exitCode: 0 }
  const r = evaluateVerification({ requiredAnchors: ['test'], evidence: [claim], candidateSha: 'abc' })
  assert.equal(r.passed, false)
  assert.ok(r.blockers.some((b) => b.includes('agent testimony')))
})

test('missing required anchor -> verification incomplete/fail, not PASS', () => {
  const r = evaluateVerification({ requiredAnchors: ['git', 'test'], evidence: [testEv(0)], candidateSha: 'abc' })
  assert.equal(r.passed, false)
  assert.ok(r.blockers.some((b) => b.includes('missing git anchor')))
})

test('wrong-SHA evidence cannot certify the current candidate', () => {
  const r = evaluateVerification({
    requiredAnchors: ['test', 'git'],
    evidence: [testEv(0, 'WRONG'), gitEv('WRONG')],
    candidateSha: 'abc',
  })
  assert.equal(r.passed, false)
  assert.ok(r.blockers.some((b) => b.includes('wrong SHA')))
})

test('valid required anchors + acceptance evidence -> PASS', () => {
  const r = evaluateVerification({ requiredAnchors: ['test', 'git'], evidence: [testEv(0), gitEv()], candidateSha: 'abc' })
  assert.deepEqual(r, { passed: true, blockers: [] })
})
