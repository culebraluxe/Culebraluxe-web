import { test } from 'node:test'
import assert from 'node:assert/strict'
import { noEvidence, promotionEligibility, qaFail, qaPass } from '../forge/evidence-gate'

// ENG-FORGE-HARDEN-03 — QA failure is repair / hard candidate evidence gate.
// NO EVIDENCE -> NO PROMOTION. FAILED EVIDENCE -> REPAIR. QA PASS for the exact
// candidate -> DEV_OPS eligible. A QA result can never certify another SHA.

test('candidate A -> QA PASS (verified A) -> DEV_OPS eligible', () => {
  const r = promotionEligibility({ candidateSha: 'A', evidence: qaPass('A', 'A') })
  assert.deepEqual(r, { eligible: true, blockers: [] })
})

test('candidate A -> QA FAIL -> NOT eligible (story stays unshipped)', () => {
  const r = promotionEligibility({ candidateSha: 'A', evidence: qaFail('A') })
  assert.equal(r.eligible, false)
  assert.ok(r.blockers.includes('QA_FAIL'))
})

test('candidate A -> QA PASS; repaired candidate B -> old A approval cannot certify B', () => {
  const approvalForA = qaPass('A', 'A')
  const b = promotionEligibility({ candidateSha: 'B', evidence: approvalForA })
  assert.equal(b.eligible, false, 'A approval must not certify B')
  assert.ok(b.blockers.includes('STALE_APPROVAL_SHA') || b.blockers.includes('VERIFIED_SHA_MISMATCH'))
})

test('candidate B -> fresh QA PASS (verified B) -> DEV_OPS eligible', () => {
  const r = promotionEligibility({ candidateSha: 'B', evidence: qaPass('B', 'B') })
  assert.deepEqual(r, { eligible: true, blockers: [] })
})

test('missing anchor evidence is incomplete, never a PASS', () => {
  const r = promotionEligibility({ candidateSha: 'A', evidence: noEvidence() })
  assert.equal(r.eligible, false)
  assert.ok(r.blockers.includes('NO_ANCHOR_EVIDENCE'))
})

test('a claimed PASS without a verified SHA anchor is not promotion-eligible', () => {
  const claimed = { evaluatedSha: 'A', qaVerdict: 'PASS' as const, verifiedSha: null, evidencePresent: true }
  const r = promotionEligibility({ candidateSha: 'A', evidence: claimed })
  assert.equal(r.eligible, false)
  assert.ok(r.blockers.includes('MISSING_VERIFIED_SHA'))
})

test('structured FAIL evidence (failed/policy/violation signals) never enables promotion', () => {
  const withFailure = { evaluatedSha: 'A', qaVerdict: 'FAIL' as const, verifiedSha: null, evidencePresent: true }
  const r = promotionEligibility({ candidateSha: 'A', evidence: withFailure })
  assert.equal(r.eligible, false)
})
