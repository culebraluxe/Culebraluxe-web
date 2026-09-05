import assert from 'node:assert/strict'
import test from 'node:test'

import { projectForgeGateFacts, type ForgeGateEvidence } from '../forge/forge-facts'

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

test('ENG-FORGE-V9: provided pass facts route the corresponding gates', () => {
  const facts = projectForgeGateFacts({ qaPassed: true, publishSucceeded: true })
  assert.equal(facts.qaPassed, true)
  assert.equal(facts.publishSucceeded, true)
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
