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

/** A real Forge story always exists; create a temp row so the start evidence FK resolves. */
async function createStory(storyId: string): Promise<void> {
  await engineSql()`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'forge smoke fixture',
      'High', 'Ready', 'temporary', 0, true
    )
    on conflict (id) do nothing
  `
}

async function cleanup(storyId: string, instanceId: string): Promise<void> {
  try {
    await engineSql()`delete from forge_workflow_evidence where process_instance_id = ${instanceId}`
    await engineSql()`delete from process_events where process_instance_id = ${instanceId}`
    await engineSql()`delete from process_instances where id = ${instanceId}`
    await engineSql()`delete from storyboard_story where id = ${storyId}`
  } catch {
    /* best-effort */
  }
}

test('ENG-FORGE-V9 smoke: architect -> smith -> QA basic path completes', async () => {
  process.env.APP_ENV = DEV
  const story = 'SMOKE-SMITH-' + Date.now()
  await createStory(story)
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

test('ENG-FORGE-V9 smoke: a HOLD decision stops the drive at the human gate (not auto-completed)', async () => {
  process.env.APP_ENV = DEV
  const story = 'SMOKE-HOLD-' + Date.now()
  await createStory(story)
  let instanceId = ''
  try {
    const res = await driveForgeStory(story, {
      start: { workType: 'FEATURE' },
      runner: chainRunner({ lead_pre: { leadDecision: 'HOLD' } }),
    })
    instanceId = res.instanceId
    assert.equal(res.needsHuman, true, 'drive must stop at the HOLD human gate')
    assert.ok(res.steps.includes('lead_pre'), 'must reach Lead PRE before HOLD')
    assert.equal(res.status, 'active', 'a held instance stays active (not completed)')
  } finally {
    if (instanceId) await cleanup(story, instanceId)
  }
})

function ev(e: ForgeGateEvidence): { transitionName: string; evidence: ForgeGateEvidence } {
  return { transitionName: 'complete', evidence: e }
}

/** Base chain runner: SMITH then QA then production smoke. */
function chainRunner(
  extra?: Record<string, ForgeGateEvidence>,
): ForgeRoleRunner {
  return async (nodeId) => {
    if (extra?.[nodeId]) return ev(extra[nodeId])
    switch (nodeId) {
      case 'lead_pre':
        return ev({ leadDecision: 'SMITH' })
      case 'qa_verify':
        return ev({
          qaPassed: true,
          publishSucceeded: true,
          migrationRequired: false,
          derivedRefreshRequired: false,
          deploymentRequired: false,
        })
      case 'production_smoke':
        return ev({ productionVerified: true })
      default:
        return ev({})
    }
  }
}

test('ENG-FORGE-V9 smoke: BUG classifies -> diagnoses -> architect/smith/QA completes', async () => {
  process.env.APP_ENV = DEV
  const story = 'SMOKE-BUG-' + Date.now()
  await createStory(story)
  let instanceId = ''
  try {
    const res = await driveForgeStory(story, {
      start: { workType: 'BUG', evidence: { rootCauseKnown: false } },
      runner: chainRunner({ diagnose_scout: { rootCauseKnown: true } }),
    })
    instanceId = res.instanceId
    assert.equal(res.status, 'completed')
    assert.ok(res.steps.includes('diagnose_scout'), 'BUG must route through diagnosis')
    assert.ok(res.steps.includes('smith'))
    assert.ok(res.steps.includes('qa_verify'))
  } finally {
    if (instanceId) await cleanup(story, instanceId)
  }
})

test('ENG-FORGE-V9 smoke: RESEARCH classifies -> archive_research (no software change)', async () => {
  process.env.APP_ENV = DEV
  const story = 'SMOKE-RESEARCH-' + Date.now()
  await createStory(story)
  let instanceId = ''
  try {
    const res = await driveForgeStory(story, {
      start: { workType: 'RESEARCH' },
      runner: chainRunner({ research_architect: { researchDisposition: 'ARCHIVE' } }),
    })
    instanceId = res.instanceId
    assert.equal(res.status, 'completed', 'research archive is a completed terminal')
    assert.ok(res.steps.includes('research_scout'))
    assert.ok(res.steps.includes('research_architect'))
    assert.ok(!res.steps.includes('smith'), 'archived research must NOT proceed to implementation')
  } finally {
    if (instanceId) await cleanup(story, instanceId)
  }
})

test('ENG-FORGE-V9 smoke: HOTFIX classifies -> straight to Lead (skips Architect) -> smith/QA', async () => {
  process.env.APP_ENV = DEV
  const story = 'SMOKE-HOTFIX-' + Date.now()
  await createStory(story)
  let instanceId = ''
  try {
    const res = await driveForgeStory(story, {
      start: { workType: 'HOTFIX', evidence: { architectureSuspect: false } },
      runner: chainRunner(),
    })
    instanceId = res.instanceId
    assert.equal(res.status, 'completed')
    assert.ok(!res.steps.includes('architect'), 'clean hotfix must skip Architect')
    assert.ok(!res.steps.includes('diagnose_scout'), 'hotfix must not diagnose')
    assert.ok(res.steps.includes('lead_pre'))
    assert.ok(res.steps.includes('smith'))
    assert.ok(res.steps.includes('qa_verify'))
  } finally {
    if (instanceId) await cleanup(story, instanceId)
  }
})
test('ENG-FORGE-V9 smoke: SPLIT fan-out rejoins before QA and completes', async () => {
  process.env.APP_ENV = DEV
  const story = 'SMOKE-SPLIT-' + Date.now()
  await createStory(story)
  let instanceId = ''
  try {
    const res = await driveForgeStory(story, {
      start: { workType: 'FEATURE' },
      runner: chainRunner({ lead_pre: { leadDecision: 'SPLIT', splitCount: 2 } }),
    })
    instanceId = res.instanceId
    assert.equal(res.status, 'completed', 'SPLIT chain must reach complete')
    const splitSteps = res.steps.filter((s) => s === 'smith_split_work').length
    assert.equal(splitSteps, 2, 'both SPLIT branches must run as Smith-split tasks')
    assert.ok(res.steps.includes('lead_post'), 'join must release Lead POST after fan-out')
    assert.ok(res.steps.includes('qa_verify'), 'QA verify after the join')
  } finally {
    if (instanceId) await cleanup(story, instanceId)
  }
})
