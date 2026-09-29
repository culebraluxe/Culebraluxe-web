import assert from 'node:assert/strict'
import test from 'node:test'

import type { AgentRunEvidence } from '@/agent-runtime/types'
import { forgeEvidenceFromAgentResult } from '@/legacy/workflow_app/forge/forge-role-mapping'
import {
  attestIntegration,
  releaseEvidenceFromIntegration,
} from '@/legacy/workflow_app/forge/forge-integration-attestation'
import {
  assessReleaseReceipt,
  deploymentReceiptFailureReason,
  isPlaceholderReceiptId,
  isRecordedDeploymentDeferral,
  releaseReceiptFromDeploymentSignal,
} from '@/legacy/workflow_app/forge/forge-release-receipt'

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

function deploy(releaseEvidence: AgentRunEvidence['releaseEvidence'], publishedSha: string | null = SHA) {
  return forgeEvidenceFromAgentResult({
    nodeId: 'deploy',
    result: result({ releaseEvidence }),
    current: publishedSha ? { publishedSha } : {},
  })
}

// --- acceptance 4: absent evidence fails closed ------------------------------

test('TECH-DEBT-07: no release receipt means the deploy gate does not pass', () => {
  const evidence = deploy(undefined)
  assert.notEqual(evidence.deploymentSucceeded, true)
  assert.equal(evidence.deploymentReceipt, undefined)
  assert.equal(evidence.failureClass, 'DEPLOYMENT')
})

test('TECH-DEBT-07: a null release evidence does not pass the deploy gate', () => {
  const evidence = deploy(null)
  assert.notEqual(evidence.deploymentSucceeded, true)
  assert.equal(evidence.failureClass, 'DEPLOYMENT')
})

// --- acceptance 2: it cannot be fabricated -----------------------------------

test('TECH-DEBT-07: a placeholder receipt id does not pass the deploy gate', () => {
  for (const receiptId of ['n/a', 'N/A', 'none', 'tbd', 'test', 'placeholder', 'waived']) {
    const evidence = deploy({ kind: 'deployment', artifactSha: SHA, receiptId, success: true })
    assert.notEqual(evidence.deploymentSucceeded, true, `receiptId ${receiptId} must not pass`)
    assert.equal(evidence.failureClass, 'DEPLOYMENT')
  }
})

test('TECH-DEBT-07: a placeholder smuggled behind prose is still a placeholder', () => {
  assert.equal(isPlaceholderReceiptId('n/a (waived by captain)'), true)
  assert.equal(isPlaceholderReceiptId('tbd - see memo'), true)
  assert.equal(isPlaceholderReceiptId('   '), true)
  assert.equal(isPlaceholderReceiptId(null), true)
  assert.equal(isPlaceholderReceiptId('vercel:dpl_9f8a7b6c5d'), false)
})

test('TECH-DEBT-07: a receipt for a different artifact does not pass', () => {
  const evidence = deploy(
    { kind: 'deployment', artifactSha: 'b'.repeat(40), receiptId: 'vercel:dpl_1', success: true },
    SHA,
  )
  assert.notEqual(evidence.deploymentSucceeded, true)
  assert.equal(evidence.failureClass, 'DEPLOYMENT')
})

test('TECH-DEBT-07: a non-sha artifact does not pass', () => {
  const evidence = deploy({ kind: 'deployment', artifactSha: 'latest', receiptId: 'vercel:dpl_1', success: true })
  assert.notEqual(evidence.deploymentSucceeded, true)
  assert.equal(evidence.failureClass, 'DEPLOYMENT')
})

test('TECH-DEBT-07: a real matching receipt still passes (no regression)', () => {
  const evidence = deploy({ kind: 'deployment', artifactSha: SHA, receiptId: 'vercel:dpl_9f8a7b6c5d', success: true })
  assert.equal(evidence.deploymentSucceeded, true)
  assert.equal(evidence.deployedSha, SHA)
  assert.equal(evidence.deploymentReceipt, 'vercel:dpl_9f8a7b6c5d')
})

// --- the producer seam: derived from a signal, never asserted ----------------

test('TECH-DEBT-07: a receipt is derived from a successful deployment signal', () => {
  const receipt = releaseReceiptFromDeploymentSignal({
    provider: 'vercel',
    deploymentId: 'dpl_9f8a7b6c5d',
    artifactSha: SHA,
    state: 'READY',
  })
  assert.deepEqual(receipt, {
    kind: 'deployment',
    artifactSha: SHA,
    receiptId: 'vercel:dpl_9f8a7b6c5d',
    success: true,
  })
  assert.equal(assessReleaseReceipt(receipt).ok, true)
  assert.equal(deploymentReceiptFailureReason(receipt, SHA), null)
})

test('TECH-DEBT-07: no signal means no receipt, so the gate stays shut', () => {
  const absent = [
    null,
    undefined,
    { provider: '', deploymentId: 'dpl_1', artifactSha: SHA, state: 'ready' },
    { provider: 'vercel', deploymentId: '', artifactSha: SHA, state: 'ready' },
    { provider: 'vercel', deploymentId: 'dpl_1', artifactSha: null, state: 'ready' },
    { provider: 'vercel', deploymentId: 'dpl_1', artifactSha: SHA, state: 'BUILDING' },
    { provider: 'vercel', deploymentId: 'dpl_1', artifactSha: SHA, state: 'ERROR' },
    { provider: 'vercel', deploymentId: 'n/a', artifactSha: SHA, state: 'ready' },
  ]
  for (const signal of absent) {
    assert.equal(releaseReceiptFromDeploymentSignal(signal), null, JSON.stringify(signal))
  }
})

test('TECH-DEBT-07: a mismatched artifact is reported with a named reason', () => {
  const reason = deploymentReceiptFailureReason(
    { kind: 'deployment', artifactSha: 'b'.repeat(40), receiptId: 'vercel:dpl_1', success: true },
    SHA,
  )
  assert.match(reason!, /deployed artifact is not the published artifact/)
  assert.match(deploymentReceiptFailureReason(null, SHA)!, /no release receipt was provided/)
  assert.match(deploymentReceiptFailureReason({ kind: 'deployment', artifactSha: SHA, receiptId: 'vercel:dpl_1', success: true }, null)!, /no published artifact sha/)
})

// --- a recorded deferral is not a claim, and an unrecorded absence is not a waiver

test('TECH-DEBT-07: only a RECORDED batch deferral counts as a deferral', () => {
  assert.equal(isRecordedDeploymentDeferral(3), true)
  assert.equal(isRecordedDeploymentDeferral(0), false)
  assert.equal(isRecordedDeploymentDeferral(null), false)
  assert.equal(isRecordedDeploymentDeferral(undefined), false)
  assert.equal(isRecordedDeploymentDeferral(Number.NaN), false)
})

// --- the deploy gate passes on a receipt the repo actually produces -----------
//
// The producer is `forge-integration-attestation.ts`: a publish-only story (no deployment
// required) is released on the integration attestation built from the published sha's
// containment in origin/main and the frozen proofs that actually ran. The gate no longer
// depends on a receipt that only a prior writer could have set.

function publishOnlyReceipt() {
  return releaseEvidenceFromIntegration(
    attestIntegration({
      candidateSha: SHA,
      integratedRef: 'origin/main',
      isAncestor: () => true,
      proofs: [
        {
          command: 'node --import tsx --test workflow_app/tests/forge-release-receipt.test.ts',
          exitCode: 0,
          durationMs: 65,
        },
      ],
    }),
  )
}

test('TECH-DEBT-07: a no-deployment story passes the deploy gate on the derived integration attestation', () => {
  const receipt = publishOnlyReceipt()
  assert.ok(receipt, 'the producer produced a receipt')
  assert.equal(receipt!.kind, 'integration')
  assert.equal(receipt!.receiptId.startsWith('integration:'), true)
  const evidence = forgeEvidenceFromAgentResult({
    nodeId: 'deploy',
    result: result({ releaseEvidence: receipt }),
    current: { publishedSha: SHA, deploymentRequired: false },
  })
  assert.equal(evidence.deploymentSucceeded, true)
  assert.equal(evidence.deployedSha, SHA)
  assert.equal(evidence.failureClass, undefined)
})

test('TECH-DEBT-07: the same publish-only attestation fails closed when the story requires a deployment', () => {
  const receipt = publishOnlyReceipt()
  const evidence = forgeEvidenceFromAgentResult({
    nodeId: 'deploy',
    result: result({ releaseEvidence: receipt }),
    current: { publishedSha: SHA, deploymentRequired: true },
  })
  assert.notEqual(evidence.deploymentSucceeded, true)
  assert.equal(evidence.deploymentReceipt, undefined)
  assert.equal(evidence.failureClass, 'DEPLOYMENT')
  assert.equal(evidence.failedReleaseStage, 'DEPLOY')
})

test('TECH-DEBT-07: with no receipt at all a no-deployment story still fails closed', () => {
  const evidence = forgeEvidenceFromAgentResult({
    nodeId: 'deploy',
    result: result({ releaseEvidence: undefined }),
    current: { publishedSha: SHA, deploymentRequired: false },
  })
  assert.notEqual(evidence.deploymentSucceeded, true)
  assert.equal(evidence.failureClass, 'DEPLOYMENT')
})
