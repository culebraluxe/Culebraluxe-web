// ---------------------------------------------------------------------------
// TECH-DEBT-07 (dialed back) — the release attestation.
//
// The point of these tests is that the attestation REFUSES more than it accepts.
// A release claim is only made when all three observable facts hold: the exact sha
// is contained in the integration ref, a build actually ran, and that build exited
// 0. Every other combination produces no evidence, which keeps the deploy gate shut
// rather than inventing a release.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  attestIntegration,
  releaseEvidenceFromIntegration,
} from '../forge/forge-integration-attestation'
import { assessReleaseReceipt, isPlaceholderReceiptId } from '../forge/forge-release-receipt'

const SHA = 'a'.repeat(40)
const REF = 'origin/main'
const BUILD_OK = { command: 'pnpm exec next build --webpack', exitCode: 0, durationMs: 74_000 }

const contained = () => true

test('attestation: a missing artifact sha is refused, not assumed', () => {
  for (const candidate of [null, undefined, '', 'not-a-sha', 'abc']) {
    const a = attestIntegration({ candidateSha: candidate, integratedRef: REF, isAncestor: contained })
    assert.equal(a.integrated, false)
    assert.equal(a.artifactSha, null)
    assert.equal(releaseEvidenceFromIntegration(a), null)
    assert.match(String(a.reason), /no artifact sha/)
  }
})

test('attestation: a sha that is NOT contained in the integration ref is refused', () => {
  const a = attestIntegration({
    candidateSha: SHA,
    integratedRef: REF,
    isAncestor: () => false,
    build: BUILD_OK,
  })
  assert.equal(a.integrated, false)
  assert.equal(a.artifactSha, SHA)
  assert.match(String(a.reason), /not contained in origin\/main/)
  assert.equal(releaseEvidenceFromIntegration(a), null, 'no integration, no receipt')
})

test('attestation: an unverifiable containment check is refused and says why', () => {
  const a = attestIntegration({
    candidateSha: SHA,
    integratedRef: REF,
    isAncestor: () => {
      throw new Error('git not available')
    },
    build: BUILD_OK,
  })
  assert.equal(a.integrated, false)
  assert.match(String(a.reason), /could not verify containment/)
  assert.match(String(a.reason), /git not available/)
})

test('attestation: integrated but no build observed → no clean-build claim', () => {
  const a = attestIntegration({ candidateSha: SHA, integratedRef: REF, isAncestor: contained })
  assert.equal(a.integrated, true)
  assert.equal(a.build, null)
  assert.match(String(a.reason), /no build was observed/)
  assert.equal(
    releaseEvidenceFromIntegration(a),
    null,
    'the captain asked for integration AND a clean build; one of the two is not enough',
  )
})

test('attestation: a build that FAILED is not a clean build', () => {
  const a = attestIntegration({
    candidateSha: SHA,
    integratedRef: REF,
    isAncestor: contained,
    build: { ...BUILD_OK, exitCode: 1 },
  })
  assert.equal(a.integrated, true)
  assert.match(String(a.reason), /build exited 1/)
  assert.equal(releaseEvidenceFromIntegration(a), null)
})

test('attestation: integration + a real clean build produces the receipt', () => {
  const a = attestIntegration({
    candidateSha: SHA,
    integratedRef: REF,
    isAncestor: contained,
    build: BUILD_OK,
  })
  assert.equal(a.reason, null)

  const evidence = releaseEvidenceFromIntegration(a)
  assert.ok(evidence, 'the receipt exists')
  assert.equal(evidence.kind, 'integration')
  assert.equal(evidence.success, true)
  assert.equal(evidence.artifactSha, SHA)
  // Prefixed so a release attestation can never be mistaken for a deployment receipt.
  assert.equal(evidence.receiptId, `integration:${REF}@${SHA.slice(0, 12)}`)
  assert.equal(isPlaceholderReceiptId(evidence.receiptId), false)
})

test('attestation: the produced receipt passes the existing release assessor', () => {
  // The gate uses the assessor, so the attestation must satisfy it as-is — being
  // "accepted by the thing that reads it" is the contract that matters.
  const evidence = releaseEvidenceFromIntegration(
    attestIntegration({
      candidateSha: SHA,
      integratedRef: REF,
      isAncestor: contained,
      build: BUILD_OK,
    }),
  )
  const assessment = assessReleaseReceipt(evidence)
  assert.equal(assessment.ok, true, JSON.stringify(assessment))
})
