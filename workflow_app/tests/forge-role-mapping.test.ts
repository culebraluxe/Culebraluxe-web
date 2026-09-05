import assert from 'node:assert/strict'
import test from 'node:test'

import type { AgentRunEvidence } from '../../agent-runtime/types'
import {
  forgeEvidenceFromAgentResult,
  forgeRoleNodePlan,
  parseForgeEvidenceMarker,
} from '../forge/forge-role-mapping'

const SHA = 'a'.repeat(40)

function result(overrides: Partial<AgentRunEvidence> = {}): AgentRunEvidence {
  return {
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
    ...overrides,
  }
}

test('ENG-FORGE-V10: every executable XML task node maps to an existing runtime lane', () => {
  const expected = {
    research_scout: 'scout',
    feature_scout: 'scout',
    diagnose_scout: 'scout',
    repair_scout: 'scout',
    research_architect: 'architect',
    architect: 'architect',
    repair_architect: 'architect',
    lead_pre: 'lead',
    lead_solo_implement: 'lead',
    lead_post: 'lead',
    failure_classifier: 'lead',
    smith: 'smith',
    smith_split_work: 'smith',
    repair_smith: 'smith',
    qa_review: 'inspector',
    qa_verify: 'assay',
    repair_devops: 'dev_ops',
    deploy: 'dev_ops',
    production_smoke: 'dev_ops',
  } as const
  for (const [nodeId, lane] of Object.entries(expected)) {
    assert.equal(forgeRoleNodePlan(nodeId).lane, lane, nodeId)
  }
})

test('ENG-FORGE-V10: free-form prose never becomes routing evidence', () => {
  assert.deepEqual(parseForgeEvidenceMarker('I think this is a CODE_DEFECT'), {})
  assert.deepEqual(
    parseForgeEvidenceMarker('FORGE_EVIDENCE_JSON: {"failureClass":"CODE_DEFECT"}'),
    { failureClass: 'CODE_DEFECT' },
  )
})

test('ENG-FORGE-V10: malformed marker values fail closed instead of poisoning gates', () => {
  assert.deepEqual(
    parseForgeEvidenceMarker(
      'FORGE_EVIDENCE_JSON: {"leadDecision":"FLY","migrationRequired":"yes","splitCount":99,"migrationFiles":["ok.sql",7],"failureClass":"CODE_DEFECT"}',
    ),
    { failureClass: 'CODE_DEFECT' },
  )
  assert.deepEqual(parseForgeEvidenceMarker('FORGE_EVIDENCE_JSON: ["CODE_DEFECT"]'), {})
})

test('ENG-FORGE-V10: Smith candidate and Lead POST integrated candidate are exact SHAs', () => {
  assert.deepEqual(
    forgeEvidenceFromAgentResult({
      nodeId: 'smith',
      result: result({ commitHash: SHA }),
      current: {},
    }),
    { candidateSha: SHA },
  )
  assert.deepEqual(
    forgeEvidenceFromAgentResult({
      nodeId: 'lead_post',
      result: result(),
      current: { candidateSha: SHA },
    }),
    { candidateSha: SHA },
  )
})

test('ENG-FORGE-V10: QA pass requires structured verification of the exact candidate', () => {
  const pass = forgeEvidenceFromAgentResult({
    nodeId: 'qa_verify',
    result: result({
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
    }),
    current: { candidateSha: SHA },
  })
  assert.equal(pass.qaPassed, true)
  assert.equal(pass.qaVerifiedSha, SHA)

  const mismatch = forgeEvidenceFromAgentResult({
    nodeId: 'qa_verify',
    result: result({
      assayEvidence: {
        version: 1,
        verdict: 'PASS',
        failureCode: null,
        failureDetail: null,
        candidateSha: 'b'.repeat(40),
        verifiedSha: 'b'.repeat(40),
        requiredCommands: ['node --test'],
        commandResults: [],
        policyViolations: [],
        startedAt: new Date(0).toISOString(),
        endedAt: new Date(1).toISOString(),
      },
    }),
    current: { candidateSha: SHA },
  })
  assert.equal(mismatch.qaPassed, false)
})

test('ENG-FORGE-V10: DEV_OPS deployment and smoke require exact-artifact receipts', () => {
  const deploy = forgeEvidenceFromAgentResult({
    nodeId: 'deploy',
    result: result({
      releaseEvidence: {
        kind: 'deployment',
        artifactSha: SHA,
        receiptId: 'deploy-123',
        success: true,
      },
    }),
    current: { publishedSha: SHA },
  })
  assert.deepEqual(deploy, {
    deploymentSucceeded: true,
    deployedSha: SHA,
    deploymentReceipt: 'deploy-123',
  })

  const smoke = forgeEvidenceFromAgentResult({
    nodeId: 'production_smoke',
    result: result({
      releaseEvidence: {
        kind: 'production_verification',
        artifactSha: SHA,
        receiptId: 'smoke-456',
        success: true,
      },
    }),
    current: { deploymentRequired: true, deployedSha: SHA },
  })
  assert.deepEqual(smoke, {
    productionVerified: true,
    productionVerifiedSha: SHA,
    productionVerificationReceipt: 'smoke-456',
  })

  const noReceipt = forgeEvidenceFromAgentResult({
    nodeId: 'deploy',
    result: result(),
    current: { publishedSha: SHA },
  })
  assert.equal(noReceipt.deploymentSucceeded, false)
  assert.equal(noReceipt.failureClass, 'DEPLOYMENT')
})
