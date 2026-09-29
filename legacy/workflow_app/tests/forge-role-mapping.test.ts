import assert from 'node:assert/strict'
import test from 'node:test'

import type { AgentRunEvidence } from '@/agent-runtime/types'
import {
  forgeEvidenceFromAgentResult,
  forgeRoleNodePlan,
  parseForgeEvidenceMarker,
} from '@/legacy/workflow_app/forge/forge-role-mapping'

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
    fast_smith: 'smith',
    fast_repair_smith: 'smith',
    qa_review: 'inspector',
    qa_verify: 'assay',
    fast_qa_verify: 'assay',
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

// EVERY LANE THAT CAN PRODUCE A CANDIDATE CARRIES IT.
//
// fast_smith and fast_repair_smith were missing from this mapping, so a FAST smith's real
// commit never reached `evidence.candidateSha`. The runner had no diff to hand the Smith
// exit gate, and the gate refused work that had landed and passed: "role did not deliver
// smith-candidate". This test walks the full set so the next lane cannot be forgotten.
test('FAST: every candidate-producing node carries its commit onto the evidence', () => {
  for (const nodeId of [
    'lead_solo_implement',
    'smith',
    'smith_split_work',
    'repair_smith',
    'fast_smith',
    'fast_repair_smith',
  ]) {
    assert.deepEqual(
      forgeEvidenceFromAgentResult({ nodeId, result: result({ commitHash: SHA }), current: {} }),
      { candidateSha: SHA },
      `${nodeId} must carry the candidate SHA it produced`,
    )
  }
})


test('ENG-FORGE-SHAPE-01: the Lead is handed the durable Architect findings', () => {
  // The field is documented as the Lead shaping gate's snapshot and was never populated,
  // so the Lead's reviewer saw zero findings and refused every valid proposal. Live on
  // 2026-09-13 that HOLDed the smoke story twice with a perfect finding in the database.
  const findings = [
    {
      id: 'F1',
      summary: 'one bounded change',
      required: true,
      seams: ['legacy/workflow_app/forge/forge-role-mapping.ts'],
      hint: 'SAME_UNIT' as const,
    },
  ]

  const mapped = forgeEvidenceFromAgentResult({
    nodeId: 'lead_pre',
    result: result(),
    current: { findings },
  })
  assert.deepEqual(mapped.findings, findings)

  // No snapshot recorded: stay absent rather than becoming a fabricated empty list, so
  // "nothing was recorded" and "an empty plan was recorded" remain distinguishable.
  assert.equal(
    forgeEvidenceFromAgentResult({ nodeId: 'lead_pre', result: result(), current: {} }).findings,
    undefined,
  )
})


test('ENG-QA-SINGLE-VERDICT-01: the QA mapping is a projector, not a second verdict author', () => {
  // This branch used to read `result.assayEvidence.verdict`/`verifiedSha` and write
  // qaPassed/qaVerifiedSha/failureClass itself. `collectAssayEvidence` (QAAgent.collect,
  // after this projection) is the writer, deriving every verdict field from
  // `adjudicateAssay`. Assert the projection contract so a reintroduced second verdict
  // computation fails here.
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
  // THE PROJECTOR PROJECTS NOTHING ABOUT THE QA VERDICT. This assertion used to REQUIRE `{ candidateSha: SHA }` on the projection, because the
  // deterministic Assay could not run without a candidate and reported a gap on every story. The lane needs
  // no candidate now, so projecting one is not a service to it — it is the git relationship that kept QA
  // unable to answer a simple question. The mapping must author no verdict AND hand the lane no identity.
  assert.deepEqual(pass, {}, 'the mapper authors no verdict and hands the lane no SHA')
  assert.equal(pass.qaPassed, undefined, 'the projector must not author a verdict')
  assert.equal(pass.qaVerifiedSha, undefined, 'the projector must not certify a SHA')
  assert.equal(pass.candidateSha, undefined, 'the projector must not hand QA a git identity')
  assert.equal(pass.verificationGap, undefined, 'and must not restore a gap flag')

  // Scope C: the FAST lane's deterministic QA node projects the same evidence.
  const fast = forgeEvidenceFromAgentResult({
    nodeId: 'fast_qa_verify',
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
  assert.deepEqual(fast, {}, 'the fast lane projects no verdict and no SHA either')

  // A durable gap flag is NOT restored either: the QA lane authors its own outcome, and a fresh read that
  // has no gap of its own must not inherit one.
  const gap = forgeEvidenceFromAgentResult({
    nodeId: 'qa_verify',
    result: result(),
    current: { candidateSha: SHA, verificationGap: true },
  })
  assert.deepEqual(gap, {})

  // A mismatched candidate in the agent-runtime evidence cannot manufacture a verdict
  // here: the projector does not read that evidence at all.
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
  assert.deepEqual(mismatch, {}, 'the projector reads no assay evidence, so no SHA can ride through it')
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
