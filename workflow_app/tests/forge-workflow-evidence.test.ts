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

test('ENG-FORGE-SHAPE-01: findings JSONB normalizes into the durable Architect snapshot', () => {
  const raw = [
    { id: 'forge', summary: 'forge seam', required: true, seams: ['workflow_app/forge/'], hint: 'SAME_UNIT' },
    { id: 'adj', summary: 'adjacent TECH', required: false, seams: ['app/tech/'], hint: 'FOLLOW_UP_STORY' },
  ]
  const fromArray = mapForgeWorkflowEvidence({ lead_decision: 'SMITH', findings: raw })
  assert.equal(fromArray.findings?.length, 2)
  assert.equal(fromArray.findings?.[0].id, 'forge')
  assert.equal(fromArray.findings?.[0].required, true)
  assert.deepEqual(fromArray.findings?.[0].seams, ['workflow_app/forge/'])
  // Postgres returns jsonb as an object; a JSON-string column value is tolerated too.
  const fromString = mapForgeWorkflowEvidence({ findings: JSON.stringify(raw) })
  assert.equal(fromString.findings?.length, 2)
  assert.equal(fromString.findings?.[1].hint, 'FOLLOW_UP_STORY')
})
