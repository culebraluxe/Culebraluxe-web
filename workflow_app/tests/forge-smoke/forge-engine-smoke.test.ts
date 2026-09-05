import { test } from 'node:test'
import assert from 'node:assert/strict'

import { driveForgeStory, type ForgeRoleRunner } from '../../forge/forge-executor'
import type { ForgeGateEvidence } from '../../forge/forge-facts'
import { engineSql } from '../../engine-client'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 — Forge engine SMOKE tests (run on demand against DEV, NOT part
// of the DB-free test:app glob).
//
//   Run: APP_ENV=development node --env-file=.env.local --import tsx \
//          --test-concurrency=1 --test workflow_app/tests/forge-smoke/*.test.ts
//
// Proves the real engine drives the old chain architect -> smith -> QA (and a
// SPLIT fan-out) to completion. Each story's engine rows are cleaned up after.
// ---------------------------------------------------------------------------

const DEV = 'development'

/** A role runner that routes execution_shape to SOLO / SMITH / SPLIT. */
function runnerFor(shape: 'SMITH' | 'SPLIT', splitCount?: number): ForgeRoleRunner {
  const leadEvidence: ForgeGateEvidence =
    shape === 'SPLIT'
      ? { leadDecision: 'SPLIT', splitCount: splitCount ?? 2 }
      : { leadDecision: shape }
  return async (nodeId) => {
    switch (nodeId) {
      case 'lead_pre':
        return { transitionName: 'complete', evidence: leadEvidence }
      case 'qa_verify':
        return {
          transitionName: 'complete',
          evidence: {
            qaPassed: true,
            publishSucceeded: true,
            migrationRequired: false,
            derivedRefreshRequired: false,
            deploymentRequired: false,
          },
        }
      case 'production_smoke':
        return { transitionName: 'complete', evidence: { productionVerified: true } }
      default:
        return { transitionName: 'complete', evidence: {} }
    }
  }
}

async function cleanup(storyId: string, instanceId: string): Promise<void> {
  try {
    await engineSql()`delete from process_events where process_instance_id = ${instanceId}`
    await engineSql()`delete from process_instances where id = ${instanceId}`
  } catch {
    /* best-effort */
  }
}

test('ENG-FORGE-V9 smoke: architect -> smith -> QA basic path completes', async () => {
  process.env.APP_ENV = DEV
  const story = 'SMOKE-SMITH-' + Date.now()
  let instanceId = ''
  try {
    const res = await driveForgeStory(story, {
      start: { workType: 'FEATURE' },
      runner: runnerFor('SMITH'),
    })
    instanceId = res.instanceId
    assert.equal(res.status, 'completed', 'SMITH chain must reach complete')
    // The old chain must pass through architect, smith and QA deterministically.
    assert.ok(res.steps.includes('architect'), 'architect step required')
    assert.ok(res.steps.includes('smith'), 'smith step required (NOT solo)')
    assert.ok(!res.steps.includes('lead_solo_implement'), 'SMITH path must not run solo')
    assert.ok(res.steps.includes('qa_verify'), 'QA verify step required')
    assert.ok(res.steps.includes('production_smoke'), 'production smoke step required')
  } finally {
    if (instanceId) await cleanup(story, instanceId)
  }
})

test('ENG-FORGE-V9 smoke: SPLIT fan-out rejoins before QA and completes', async () => {
  process.env.APP_ENV = DEV
  const story = 'SMOKE-SPLIT-' + Date.now()
  let instanceId = ''
  try {
    const res = await driveForgeStory(story, {
      start: { workType: 'FEATURE' },
      runner: runnerFor('SPLIT', 2),
    })
    instanceId = res.instanceId
    assert.equal(res.status, 'completed', 'SPLIT chain must reach complete')
    assert.ok(res.steps.includes('architect'))
    // SPLIT branches rejoin at split_join and only then reach Lead POST -> QA.
    assert.ok(res.steps.includes('lead_post'), 'join must release Lead POST after fan-out')
    assert.ok(res.steps.includes('qa_verify'), 'QA verify after the join')
  } finally {
    if (instanceId) await cleanup(story, instanceId)
  }
})
