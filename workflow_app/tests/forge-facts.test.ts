import assert from 'node:assert/strict'
import test from 'node:test'

import {
  forgeFastEligibility,
  forgeLineageError,
  projectForgeGateFacts,
  type ForgeGateEvidence,
} from '../forge/forge-facts'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 Item 2 — decision-gate facts projection (pure).
// ---------------------------------------------------------------------------

test('ENG-FORGE-V9: workType routes classify_work and is carried verbatim', () => {
  const facts = projectForgeGateFacts({ workType: 'FEATURE' })
  assert.equal(facts.workType, 'FEATURE')
})

test('ENG-FORGE-V9: boolean gates default false (story holds until positive evidence)', () => {
  const facts = projectForgeGateFacts({ workType: 'FEATURE' })
  assert.equal(facts.qaPassed, false)
  assert.equal(facts.publishSucceeded, false)
  assert.equal(facts.productionVerified, false)
  assert.equal(facts.deploymentSucceeded, false)
})

test('ENG-FORGE-V10: provided pass facts route only with exact candidate lineage', () => {
  const sha = 'a'.repeat(40)
  const facts = projectForgeGateFacts({
    qaPassed: true,
    publishSucceeded: true,
    candidateSha: sha,
    qaVerifiedSha: sha,
    publishedSha: sha,
  })
  assert.equal(facts.qaPassed, true)
  assert.equal(facts.publishSucceeded, true)
})

test('ENG-FORGE-V10: boolean success cannot bypass missing or mismatched SHA lineage', () => {
  assert.equal(projectForgeGateFacts({ qaPassed: true }).qaPassed, false)
  const candidate = 'b'.repeat(40)
  const other = 'c'.repeat(40)
  const evidence: ForgeGateEvidence = {
    qaPassed: true,
    publishSucceeded: true,
    candidateSha: candidate,
    qaVerifiedSha: other,
    publishedSha: candidate,
  }
  assert.match(forgeLineageError(evidence, 'qa') ?? '', /expected candidate/)
  assert.equal(projectForgeGateFacts(evidence).qaPassed, false)
  assert.equal(projectForgeGateFacts(evidence).publishSucceeded, false)
})

test('ENG-FORGE-V10: deployment and production verification enforce the same artifact', () => {
  const sha = 'd'.repeat(40)
  const evidence: ForgeGateEvidence = {
    candidateSha: sha,
    qaVerifiedSha: sha,
    publishedSha: sha,
    deployedSha: sha,
    productionVerifiedSha: sha,
    deploymentRequired: true,
    deploymentSucceeded: true,
    productionVerified: true,
  }
  assert.equal(forgeLineageError(evidence, 'production'), null)
  assert.equal(projectForgeGateFacts(evidence).deploymentSucceeded, true)
  assert.equal(projectForgeGateFacts(evidence).productionVerified, true)
})

test('ENG-FORGE-V9: enum router facts are NOT fabricated when absent (fail closed)', () => {
  const facts = projectForgeGateFacts({})
  assert.equal(facts.failureClass, undefined)
  assert.equal(facts.resumeTarget, undefined)
  assert.equal(facts.leadDecision, undefined)
})

test('ENG-FORGE-V9: splitCount and hold resume target project', () => {
  const ev: ForgeGateEvidence = { leadDecision: 'SPLIT', splitCount: 3 }
  const facts = projectForgeGateFacts(ev)
  assert.equal(facts.leadDecision, 'SPLIT')
  assert.equal(facts.splitCount, 3)
})

test('Scope C: FAST eligibility fails closed when any release obligation appears', () => {
  assert.equal(forgeFastEligibility({ workType: 'FAST' }), true)
  assert.equal(projectForgeGateFacts({ workType: 'FAST' }).fastEligible, true)
  // Any release/schema obligation makes FAST ineligible (never a hidden release).
  assert.equal(forgeFastEligibility({ workType: 'FAST', migrationRequired: true }), false)
  assert.equal(forgeFastEligibility({ workType: 'FAST', derivedRefreshRequired: true }), false)
  assert.equal(forgeFastEligibility({ workType: 'FAST', deploymentRequired: true }), false)
  assert.equal(forgeFastEligibility({ workType: 'FAST', architectureSuspect: true }), false)
  assert.equal(forgeFastEligibility({ workType: 'FAST', leadDecision: 'SPLIT' }), false)
  // Non-FAST work is never FAST-eligible.
  assert.equal(forgeFastEligibility({ workType: 'FEATURE' }), false)
})
