import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FORGE_MIGRATE_DEV,
  FORGE_PUBLISH_CANDIDATE,
  FORGE_REFRESH_DERIVED_MODELS,
  FORGE_VERIFY_DEV_MIGRATION,
} from '../forge-command-types'
import { createDbForgeReleaseExecutor } from '../forge/db-release-executor'
import type { ForgeGateEvidence } from '../forge/forge-facts'
import type { ForgeReleaseOperations } from '../forge/release-operations'
import { forgeReleaseSafety } from '../forge/release-operations'

const SHA = 'a'.repeat(40)

function command() {
  return {
    commandId: 'cmd-1',
    commandType: FORGE_PUBLISH_CANDIDATE,
    processInstanceId: 'instance-1',
    storyId: 'STORY-1',
    input: {},
  }
}

test('ENG-FORGE-V10: publish command publishes and records the exact QA-approved SHA', async () => {
  const writes: ForgeGateEvidence[] = []
  const executor = createDbForgeReleaseExecutor('/repo', {
    readEvidence: async () => ({ candidateSha: SHA, qaVerifiedSha: SHA, qaPassed: true }),
    mergeEvidence: async (_instance, _story, evidence) => {
      writes.push(evidence)
    },
    publish: async (input) => {
      assert.equal(input.repoRoot, '/repo')
      assert.equal(input.candidateCommit, SHA)
      return { outcome: 'published', candidateCommit: SHA, publishedMainHash: SHA }
    },
  })

  const result = await executor.execute(command())
  assert.equal(result.outcome, 'success')
  assert.deepEqual(writes, [{ publishSucceeded: true, publishedSha: SHA }])
})

test('ENG-FORGE-V10: publish refuses a candidate not verified by QA', async () => {
  const writes: ForgeGateEvidence[] = []
  let publishCalls = 0
  const executor = createDbForgeReleaseExecutor('/repo', {
    readEvidence: async () => ({ candidateSha: SHA, qaVerifiedSha: 'b'.repeat(40), qaPassed: true }),
    mergeEvidence: async (_instance, _story, evidence) => {
      writes.push(evidence)
    },
    publish: async () => {
      publishCalls += 1
      return { outcome: 'published', candidateCommit: SHA, publishedMainHash: SHA }
    },
  })

  const result = await executor.execute(command())
  assert.equal(result.outcome, 'success', 'business failure must route through XML repair')
  assert.equal(publishCalls, 0)
  assert.deepEqual(writes, [
    {
      publishSucceeded: false,
      failureClass: 'PUBLISH_CONFLICT',
      failedReleaseStage: 'PUBLISH',
    },
  ])
})

test('ENG-FORGE-V10: rejected fast-forward records repair evidence, never fake success', async () => {
  const writes: ForgeGateEvidence[] = []
  const executor = createDbForgeReleaseExecutor('/repo', {
    readEvidence: async () => ({ candidateSha: SHA, qaVerifiedSha: SHA, qaPassed: true }),
    mergeEvidence: async (_instance, _story, evidence) => {
      writes.push(evidence)
    },
    publish: async () => ({
      outcome: 'publish-conflict',
      candidateCommit: SHA,
      remoteMainHash: 'b'.repeat(40),
      reason: 'origin/main advanced',
    }),
  })

  const result = await executor.execute(command())
  assert.equal(result.outcome, 'success')
  assert.match(result.message ?? '', /advanced/)
  assert.equal(writes[0]?.publishSucceeded, false)
  assert.equal(writes[0]?.failureClass, 'PUBLISH_CONFLICT')
})

function fakeOperations(calls: string[]): ForgeReleaseOperations {
  return {
    async applyMigrations(input) {
      calls.push(`apply:${input.target}:${input.migrationFiles.join(',')}`)
      return { success: true, detail: 'applied' }
    },
    async verifyMigrations(input) {
      calls.push(`verify:${input.target}:${input.migrationFiles.join(',')}`)
      return { success: true, detail: 'verified' }
    },
    async refreshDerived(input) {
      calls.push(`refresh:${input.target}:${input.models.join(',')}`)
      return { success: true, detail: 'refreshed' }
    },
    async verifyDerived(input) {
      calls.push(`verify-derived:${input.target}:${input.models.join(',')}`)
      return { success: true, detail: 'verified derived' }
    },
  }
}

test('ENG-FORGE-V10: migration apply and verify are real separate operations with evidence', async () => {
  const calls: string[] = []
  const writes: ForgeGateEvidence[] = []
  const executor = createDbForgeReleaseExecutor('/repo', {
    readEvidence: async () => ({ migrationRequired: true, migrationFiles: ['108_demo.sql'] }),
    mergeEvidence: async (_instance, _story, evidence) => {
      writes.push(evidence)
    },
    operations: fakeOperations(calls),
  })
  assert.equal((await executor.execute({ ...command(), commandType: FORGE_MIGRATE_DEV })).outcome, 'success')
  assert.equal(
    (await executor.execute({ ...command(), commandType: FORGE_VERIFY_DEV_MIGRATION })).outcome,
    'success',
  )
  assert.deepEqual(calls, ['apply:dev:108_demo.sql', 'verify:dev:108_demo.sql'])
  assert.deepEqual(writes, [{ devMigrationApplied: true }, { devMigrationVerified: true }])
})

test('ENG-FORGE-V10: derived refresh command uses the declared model plan', async () => {
  const calls: string[] = []
  const writes: ForgeGateEvidence[] = []
  const executor = createDbForgeReleaseExecutor('/repo', {
    readEvidence: async () => ({ derivedRefreshRequired: true, derivedModels: ['public.mv_clients'] }),
    mergeEvidence: async (_instance, _story, evidence) => {
      writes.push(evidence)
    },
    operations: fakeOperations(calls),
  })
  const result = await executor.execute({ ...command(), commandType: FORGE_REFRESH_DERIVED_MODELS })
  assert.equal(result.outcome, 'success')
  assert.deepEqual(calls, ['refresh:prod:public.mv_clients'])
  assert.deepEqual(writes, [{ derivedRefreshSucceeded: true }])
})

test('ENG-FORGE-V10: release operations reject path traversal and unsafe SQL identifiers', () => {
  assert.throws(() => forgeReleaseSafety.migrationPath('/repo', '../secrets.sql'), /unsafe/)
  assert.throws(() => forgeReleaseSafety.quotedModel('mv; drop table x'), /unsafe/)
  assert.equal(forgeReleaseSafety.quotedModel('public.mv_clients'), '"public"."mv_clients"')
})
