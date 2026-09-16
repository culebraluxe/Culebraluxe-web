import { test } from 'node:test'
import assert from 'node:assert/strict'
import { DevOpsAgent } from '../forge/agents/role-agents'
import type { ReleaseEvidencePort } from '../forge/agents/ports'
import type { ForgeGateEvidence } from '../forge/forge-facts'

// A release receipt is recorded by its KIND, never by elimination. A publish
// (`integration`) must never be written as a deployment, and an unlisted kind is
// not a deployment either.
const agent = new DevOpsAgent('production_smoke')

const collect = (receipt: ReleaseEvidencePort): ForgeGateEvidence =>
  agent.collect({} as ForgeGateEvidence, '', { releaseEvidence: receipt })

test('an integration receipt records neither deploymentReceipt nor deployedSha', () => {
  const ev = collect({
    kind: 'integration',
    success: true,
    receiptId: 'push:08b569f8',
    artifactSha: '08B569F8',
  })
  assert.equal(ev.deploymentReceipt, undefined)
  assert.equal(ev.deployedSha, undefined)
  assert.equal(ev.productionVerificationReceipt, undefined)
  assert.equal(ev.productionVerifiedSha, undefined)
})

test('a deployment receipt records deploymentReceipt and deployedSha', () => {
  const ev = collect({ kind: 'deployment', success: true, receiptId: 'd-1', artifactSha: 'ABC123' })
  assert.equal(ev.deploymentReceipt, 'd-1')
  assert.equal(ev.deployedSha, 'abc123')
  assert.equal(ev.productionVerificationReceipt, undefined)
  assert.equal(ev.productionVerifiedSha, undefined)
})

test('a production_verification receipt records the production receipt and sha only', () => {
  const ev = collect({
    kind: 'production_verification',
    success: true,
    receiptId: 'pv-1',
    artifactSha: 'DEF456',
  })
  assert.equal(ev.productionVerificationReceipt, 'pv-1')
  assert.equal(ev.productionVerifiedSha, 'def456')
  assert.equal(ev.deploymentReceipt, undefined)
  assert.equal(ev.deployedSha, undefined)
})

test('an unlisted kind is never recorded as a deployment', () => {
  const ev = collect({
    kind: 'publish' as ReleaseEvidencePort['kind'],
    success: true,
    receiptId: 'x-1',
    artifactSha: 'AA',
  })
  assert.equal(ev.deploymentReceipt, undefined)
  assert.equal(ev.deployedSha, undefined)
  assert.equal(ev.productionVerificationReceipt, undefined)
  assert.equal(ev.productionVerifiedSha, undefined)
})

test('a failed or empty receipt records nothing regardless of kind', () => {
  const failed = collect({ kind: 'deployment', success: false, receiptId: 'd-1', artifactSha: 'ABC' })
  assert.equal(failed.deploymentReceipt, undefined)
  const empty = collect({ kind: 'deployment', success: true, receiptId: '   ', artifactSha: 'ABC' })
  assert.equal(empty.deploymentReceipt, undefined)
})
