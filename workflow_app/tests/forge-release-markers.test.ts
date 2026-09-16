import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FORGE_MIGRATE_DEV,
  FORGE_PUBLISH_CANDIDATE,
  FORGE_REFRESH_DERIVED_MODELS,
} from '../forge-command-types'
import { createDbForgeReleaseExecutor } from '../forge/db-release-executor'
import type { ForgeEvidenceMerge } from '../../db/forge-workflow-evidence'
import type { ForgeGateEvidence } from '../forge/forge-facts'
import type { ForgeReleaseOperations } from '../forge/release-operations'

// ---------------------------------------------------------------------------
// ENG-FORGE-RELEASE-MARKERS-01: a resolved release stops reporting the failure it
// resolved.
//
// The executor records the resolution; the repository merge honors it. This test
// MODELS the real on-conflict semantics (`coalesce` preserves a null/omitted value;
// `releaseFailureResolved: true` clears the three markers) instead of only capturing
// the executor's payload, so a pass is evidence about the durable row and not about
// the shape of one write.
// ---------------------------------------------------------------------------

const SHA = 'a'.repeat(40)

function command(commandType: string) {
  return {
    commandId: 'cmd-1',
    commandType,
    processInstanceId: 'instance-1',
    storyId: 'STORY-1',
    input: {},
  }
}

/** Mirror of mergeForgeWorkflowEvidence's on-conflict behavior for the marker columns. */
function mergeRow(stored: ForgeGateEvidence, evidence: ForgeEvidenceMerge): ForgeGateEvidence {
  const next: Record<string, unknown> = { ...stored }
  for (const [key, value] of Object.entries(evidence)) {
    if (key === 'releaseFailureResolved') continue
    if (value === undefined || value === null) continue // coalesce: preserve
    next[key] = value
  }
  if (evidence.releaseFailureResolved === true) {
    next.failureClass = null
    next.failedReleaseStage = null
    next.lastFailure = null
  }
  return next as ForgeGateEvidence
}

function mutableOperations() {
  let outcome = { success: true, detail: 'ok' }
  const operations: ForgeReleaseOperations = {
    async applyMigrations() {
      return { ...outcome }
    },
    async verifyMigrations() {
      return { ...outcome }
    },
    async refreshDerived() {
      return { ...outcome }
    },
    async verifyDerived() {
      return { ...outcome }
    },
  }
  return {
    operations,
    set: (next: { success: boolean; detail?: string }) => {
      outcome = { ...next }
    },
  }
}

test('ENG-FORGE-RELEASE-MARKERS: a resolved DEV migration clears the markers and keeps the success flag', async () => {
  let row: ForgeGateEvidence = {}
  const ops = mutableOperations()
  ops.set({ success: false, detail: 'apply failed' })
  const executor = createDbForgeReleaseExecutor('/repo', {
    readEvidence: async () => ({ ...row }),
    mergeEvidence: async (_instance, _story, evidence) => {
      row = mergeRow(row, evidence)
    },
    operations: ops.operations,
  })

  await executor.execute(command(FORGE_MIGRATE_DEV))
  assert.equal(row.failureClass, 'MIGRATION')
  assert.equal(row.failedReleaseStage, 'DEV_MIGRATION')
  assert.equal(row.devMigrationApplied, false)

  ops.set({ success: true, detail: 'applied' })
  await executor.execute(command(FORGE_MIGRATE_DEV))
  assert.equal(row.failureClass, null)
  assert.equal(row.failedReleaseStage, null)
  assert.equal(row.lastFailure, null)
  assert.equal(row.devMigrationApplied, true)
})

test('ENG-FORGE-RELEASE-MARKERS: a failure not yet followed by a success keeps its markers', async () => {
  let row: ForgeGateEvidence = {}
  const ops = mutableOperations()
  ops.set({ success: false, detail: 'apply failed' })
  const executor = createDbForgeReleaseExecutor('/repo', {
    readEvidence: async () => ({ ...row }),
    mergeEvidence: async (_instance, _story, evidence) => {
      row = mergeRow(row, evidence)
    },
    operations: ops.operations,
  })

  await executor.execute(command(FORGE_MIGRATE_DEV))
  assert.equal(row.failureClass, 'MIGRATION')
  assert.equal(row.failedReleaseStage, 'DEV_MIGRATION')
})

test('ENG-FORGE-RELEASE-MARKERS: a different stage success does not clear an unresolved stage', async () => {
  let row: ForgeGateEvidence = { failureClass: 'MIGRATION', failedReleaseStage: 'DEV_MIGRATION' }
  const ops = mutableOperations()
  ops.set({ success: true, detail: 'refreshed' })
  const executor = createDbForgeReleaseExecutor('/repo', {
    readEvidence: async () => ({ ...row }),
    mergeEvidence: async (_instance, _story, evidence) => {
      row = mergeRow(row, evidence)
    },
    operations: ops.operations,
  })

  await executor.execute(command(FORGE_REFRESH_DERIVED_MODELS))
  assert.equal(row.derivedRefreshSucceeded, true)
  assert.equal(row.failureClass, 'MIGRATION')
  assert.equal(row.failedReleaseStage, 'DEV_MIGRATION')
})

test('ENG-FORGE-RELEASE-MARKERS: a resolved publish clears the markers it resolved', async () => {
  let row: ForgeGateEvidence = {
    candidateSha: SHA,
    qaPassed: true,
    failureClass: 'PUBLISH_CONFLICT',
    failedReleaseStage: 'PUBLISH',
    lastFailure: 'origin/main advanced',
  }
  const executor = createDbForgeReleaseExecutor('/repo', {
    readEvidence: async () => ({ ...row }),
    mergeEvidence: async (_instance, _story, evidence) => {
      row = mergeRow(row, evidence)
    },
    publish: async () => ({ outcome: 'published', candidateCommit: SHA, publishedMainHash: SHA }),
  })

  await executor.execute(command(FORGE_PUBLISH_CANDIDATE))
  assert.equal(row.publishSucceeded, true)
  assert.equal(row.publishedSha, SHA)
  assert.equal(row.failureClass, null)
  assert.equal(row.failedReleaseStage, null)
  assert.equal(row.lastFailure, null)
})

test('ENG-FORGE-RELEASE-MARKERS: a publish failure still records its markers and reason', async () => {
  let row: ForgeGateEvidence = { candidateSha: SHA, qaPassed: true }
  const executor = createDbForgeReleaseExecutor('/repo', {
    readEvidence: async () => ({ ...row }),
    mergeEvidence: async (_instance, _story, evidence) => {
      row = mergeRow(row, evidence)
    },
    publish: async () => ({
      outcome: 'publish-conflict',
      candidateCommit: SHA,
      remoteMainHash: 'b'.repeat(40),
      reason: 'origin/main advanced',
    }),
  })

  await executor.execute(command(FORGE_PUBLISH_CANDIDATE))
  assert.equal(row.publishSucceeded, false)
  assert.equal(row.failureClass, 'PUBLISH_CONFLICT')
  assert.equal(row.failedReleaseStage, 'PUBLISH')
  assert.equal(row.lastFailure, 'origin/main advanced')
})
