import assert from 'node:assert/strict'
import test from 'node:test'

import { mapForgeWorkflowEvidence } from '../../db/forge-workflow-evidence'
import { projectForgeGateFacts } from '../forge/forge-facts'

test('ENG-FORGE-V10: durable evidence rows normalize into the stable Forge contract', () => {
  const sha = 'a'.repeat(40)
  const evidence = mapForgeWorkflowEvidence({
    work_type: 'FEATURE',
    lead_decision: 'SMITH',
    qa_passed: true,
    publish_succeeded: true,
    candidate_sha: sha,
    qa_verified_sha: sha,
    published_sha: sha,
    deployment_receipt: 'deploy-123',
    production_verification_receipt: 'smoke-456',
  })
  assert.equal(evidence.workType, 'FEATURE')
  assert.equal(evidence.leadDecision, 'SMITH')
  assert.equal(projectForgeGateFacts(evidence).qaPassed, true)
  assert.equal(projectForgeGateFacts(evidence).publishSucceeded, true)
  assert.equal(evidence.deploymentReceipt, 'deploy-123')
  assert.equal(evidence.productionVerificationReceipt, 'smoke-456')
})

test('ENG-FORGE-V10: null database values remain unknown rather than fabricated facts', () => {
  const evidence = mapForgeWorkflowEvidence({
    work_type: 'BUG',
    root_cause_known: null,
    qa_passed: null,
  })
  assert.equal(evidence.rootCauseKnown, undefined)
  assert.equal(evidence.qaPassed, undefined)
  assert.equal(projectForgeGateFacts(evidence).qaPassed, false)
})
