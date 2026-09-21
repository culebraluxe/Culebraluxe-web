// ---------------------------------------------------------------------------
// ENG-FORGE-DEPLOY-NOMECH-01 — FORGE RECORDS NO DEPLOYMENT IT DID NOT PERFORM.
//
// Deployment is not something Forge does: nothing in this repository mints a deployment receipt,
// and the deploy script is run by the captain at sprint release. So the deploy entry must decide
// from durable evidence BEFORE the lane runs: a required deployment with no producer is held at
// the decision with a named reason, a batch-deferred story completes with no receipt, and nothing
// records a deployed sha unless a deployment actually happened.
//
// No database, no OpenCode: the decision is pure, and the wiring is read from the runner source
// (the same drift detector forge-smith-door.test.ts uses).
// ---------------------------------------------------------------------------

import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import test from 'node:test'

import {
  FORGE_DEPLOY_NO_PRODUCER_REASON,
  forgeDeployHoldReason,
  forgeDeploymentProducerConfigured,
  projectForgeGateFacts,
} from '@/legacy/workflow_app/forge/forge-facts'
import type { ForgeGateEvidence } from '@/legacy/workflow_app/forge/forge-facts'
import { deriveReleaseEvidence } from '@/legacy/workflow_app/forge/agent-runtime-role-runner'

const RUNNER_SRC = readFileSync(
  join(process.cwd(), 'legacy/workflow_app/forge/agent-runtime-role-runner.ts'),
  'utf8',
)
const SHA = 'a'.repeat(40)
const facts = (evidence: ForgeGateEvidence): Record<string, unknown> =>
  projectForgeGateFacts(evidence) as Record<string, unknown>

test('deferred deployment completes with no deployment receipt', () => {
  const evidence: ForgeGateEvidence = { deploymentRequired: true, deploymentDeferredToBatch: 92 }

  assert.equal(forgeDeployHoldReason(evidence), null, 'a recorded deferral is an accepted outcome')
  assert.equal(forgeDeploymentProducerConfigured(evidence), false)

  const f = facts(evidence)
  assert.equal(f.deploymentDeferred, true)
  assert.equal(f.releaseDeferred, true)
  assert.equal(f.deploymentBlocked, false, 'a deferred story is never blocked at the decision')
  assert.equal(f.deploymentSucceeded, false, 'a deferral is not a deployment')
  assert.equal(f.deploymentReceipt, undefined, 'a deferral records no deployment receipt')
  assert.equal(f.deployedSha, undefined, 'a deferral records no deployed sha')
})

test('required deployment with no producer is held at the decision, naming what is missing', () => {
  const required: ForgeGateEvidence = { deploymentRequired: true, candidateSha: SHA, qaPassed: true }

  const reason = forgeDeployHoldReason(required)
  assert.equal(typeof reason, 'string', 'a required deployment with no producer must be named')
  assert.equal(reason, FORGE_DEPLOY_NO_PRODUCER_REASON)
  assert.match(reason ?? '', /no deployment producer is configured/i)
  assert.equal(facts(required).deploymentBlocked, true)

  // A receipt with no sha attests nothing: the producer fact needs BOTH the receipt and the sha.
  assert.equal(
    forgeDeploymentProducerConfigured({ deploymentRequired: true, deploymentReceipt: 'deploy:abc' }),
    false,
  )
  assert.notEqual(
    forgeDeployHoldReason({ deploymentRequired: true, deploymentReceipt: 'deploy:abc' }),
    null,
  )

  // A durable deployment receipt IS the producer, and it releases the decision.
  const produced: ForgeGateEvidence = {
    deploymentRequired: true,
    deploymentReceipt: 'deploy:abc',
    deployedSha: SHA,
  }
  assert.equal(forgeDeploymentProducerConfigured(produced), true)
  assert.equal(forgeDeployHoldReason(produced), null)
  assert.equal(facts(produced).deploymentBlocked, false)

  // A story that does not demand a deployment is never blocked.
  assert.equal(forgeDeployHoldReason({ deploymentRequired: false }), null)
  assert.equal(facts({ deploymentRequired: false }).deploymentBlocked, false)
})

test('the deploy entry consults the decision before the deploy lane runs', () => {
  const guardAt = RUNNER_SRC.indexOf('forgeDeployHoldReason(await readForgeWorkflowEvidence')
  const deferralAt = RUNNER_SRC.indexOf('deploymentDeferredToBatch: deferredTo')
  const laneAt = RUNNER_SRC.indexOf('executeClaimedAgentCommand(')

  assert.ok(guardAt > 0, 'the deploy entry must consult forgeDeployHoldReason')
  assert.ok(
    deferralAt > 0 && deferralAt < guardAt,
    'the guard must sit AFTER the batch-deferral early return or it would hold every deferred story',
  )
  assert.ok(laneAt > guardAt, 'the guard must sit BEFORE the lane command so the lane never runs')
  assert.match(RUNNER_SRC, /resumeTarget: 'DEPLOY'/, 'the hold must name the lane it never entered')
  assert.match(RUNNER_SRC, /originatingNode: nodeId/)
  assert.match(
    RUNNER_SRC,
    /current\.deploymentReceipt && evidence\.deploymentRequired === true/,
    'the deployment mirror must be gated on a story that requires a deployment',
  )
})

test('no deployment fact is recorded unless a deployment actually happened', async () => {
  const base = { nodeId: 'deploy', publishedSha: null, proofs: [], cwd: process.cwd() }
  const none = {
    deploymentReceipt: null,
    deployedSha: null,
    productionVerificationReceipt: null,
    productionVerifiedSha: null,
  }

  // A required deployment with no producer mints NO receipt at all.
  assert.equal(await deriveReleaseEvidence({ ...base, ...none, deploymentRequired: true }), null)

  // A story that does not require a deployment never yields a deployment-kind receipt.
  const noDeploy = await deriveReleaseEvidence({ ...base, ...none, deploymentRequired: false })
  assert.notEqual(noDeploy?.kind, 'deployment')

  // Only a durable deployment receipt produces a deployment receipt.
  const deployed = await deriveReleaseEvidence({
    ...base,
    deploymentRequired: true,
    deploymentReceipt: 'deploy:abc',
    deployedSha: SHA,
    productionVerificationReceipt: null,
    productionVerifiedSha: null,
  })
  assert.equal(deployed?.kind, 'deployment')
  assert.equal(deployed?.artifactSha, SHA)
})
