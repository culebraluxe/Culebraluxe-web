import assert from 'node:assert/strict'
import test from 'node:test'

import type { AgentRunEvidence } from '../../agent-runtime/types'
import { forgeEvidenceFromAgentResult } from '../forge/forge-role-mapping'
import { FailureClassifierAgent } from '../forge/agents/role-agents'

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

// ENG-FORGE-FAILURE-LABEL-01 — a release failure keeps the class its own stage recorded.
//
// FOUND 2026-09-16 on ENG-FORGE-RECEIPT-KIND-01: the publisher recorded
// failureClass=PUBLISH_CONFLICT with failedReleaseStage=PUBLISH, then the classifier replaced it
// with ENVIRONMENT. The router reads failedReleaseStage, so a replaced class is a wrong record
// of why the release failed.
test('ENG-FORGE-FAILURE-LABEL-01: a stage-recorded class survives the classifier and its label is metadata', () => {
  const classifier = new FailureClassifierAgent('failure_classifier')
  const mapped = forgeEvidenceFromAgentResult({
    nodeId: 'failure_classifier',
    result: result({ notes: 'FORGE_EVIDENCE_JSON: {"failureClass":"ENVIRONMENT"}' }),
    current: { failureClass: 'PUBLISH_CONFLICT', failedReleaseStage: 'PUBLISH' },
  })

  assert.equal(mapped.failureClass, 'PUBLISH_CONFLICT')
  assert.equal(mapped.failedReleaseStage, 'PUBLISH')

  const collected = classifier.collect(mapped, 'FAILURE_CLASS: ENVIRONMENT')
  assert.equal(collected.failureClass, 'PUBLISH_CONFLICT')
  assert.equal(collected.failedReleaseStage, 'PUBLISH')
  assert.equal(collected.classifierFailureClass, 'ENVIRONMENT')
})

// The classifier is the LAST writer, so the guard must hold even when its own marker carries a
// label and the stage class was only in `failureClass`.
test('ENG-FORGE-FAILURE-LABEL-01: the classifier cannot replace a stage class even when its marker carries one', () => {
  const classifier = new FailureClassifierAgent('failure_classifier')
  const collected = classifier.collect(
    { failureClass: 'PUBLISH_CONFLICT', failedReleaseStage: 'PUBLISH' },
    'FORGE_EVIDENCE_JSON: {"failureClass":"ENVIRONMENT"}\nFAILURE_CLASS: ENVIRONMENT',
  )

  assert.equal(collected.failureClass, 'PUBLISH_CONFLICT')
  assert.equal(collected.classifierFailureClass, 'ENVIRONMENT')
})

// A deploy-stage failure records DEPLOYMENT/DEPLOY the same way; the guard is not publish-only.
test('ENG-FORGE-FAILURE-LABEL-01: a deploy-stage class is preserved too', () => {
  const mapped = forgeEvidenceFromAgentResult({
    nodeId: 'failure_classifier',
    result: result({ notes: 'FAILURE_CLASS: CODE_DEFECT' }),
    current: { failureClass: 'DEPLOYMENT', failedReleaseStage: 'DEPLOY' },
  })

  assert.equal(mapped.failureClass, 'DEPLOYMENT')
  assert.equal(mapped.failedReleaseStage, 'DEPLOY')
  assert.equal(mapped.stageFailureClass, 'DEPLOYMENT')
})

// With no stage-recorded class the classifier label stands exactly as it does today.
test('ENG-FORGE-FAILURE-LABEL-01: with no stage-recorded class the classifier label stands', () => {
  const classifier = new FailureClassifierAgent('failure_classifier')
  const mapped = forgeEvidenceFromAgentResult({
    nodeId: 'failure_classifier',
    result: result({ notes: 'FORGE_EVIDENCE_JSON: {"failureClass":"ENVIRONMENT"}' }),
    current: {},
  })

  assert.deepEqual(mapped, { failureClass: 'ENVIRONMENT' })

  const collected = classifier.collect(mapped, 'FAILURE_CLASS: CODE_DEFECT')
  assert.equal(collected.failureClass, 'CODE_DEFECT')
  assert.equal(collected.classifierFailureClass, undefined)
})
