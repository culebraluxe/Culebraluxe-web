import { test } from 'node:test'
import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'

import {
  applyForgeCompletionUnit,
  claimForgeRoleTask,
  completeForgeRoleTask,
  forgeCompletionReceiptId,
  listActiveForgeRoleTasks,
  reconcileForgeCompletions,
  startForgeWorkflow,
} from '../forge/forge-engine-runtime'
import type { ForgeGateEvidence } from '../forge/forge-facts'
import { engineSql } from '../engine-client'
import { FORGE_SDLC_KEY, FORGE_SDLC_VERSION, parseForgeSdlc } from '../definitions/forge-sdlc'
import { readForgeWorkflowEvidence } from '../../db/forge-workflow-evidence'
import { readForgeRepairLedger } from '../../db/forge-repair-ledger'
import { readFinalReceipt } from '../../db/workflow-command-receipt'

// ---------------------------------------------------------------------------
// ENG-FORGE-CRASH-WINDOW-01 — the completion unit is recoverable and exactly-once.
//
// Run (DEV, on demand — the DB-free glob does not include it):
//   node --env-file=.env.local --import tsx --test workflow_app/tests/completion-crash-window.test.ts
//
// Two crash windows used to be unrecoverable:
//   1. the engine transition committed but the evidence merge did not — an advanced
//      workflow with no result; and
//   2. the evidence committed but the repair counter did not — an undercounted repair.
// Both are now the SAME unit (applyForgeCompletionUnit): one claim-first receipt
// transaction covering the evidence merge AND the counter. A crash rolls the whole
// unit back, leaving no receipt, and a later run FINISHES it from the engine's own
// durable `task.completed` record instead of re-running the role.
// ---------------------------------------------------------------------------

// The proof is a DEV fixture story (temp rows, cleaned up) — declare the target
// explicitly so the engine and the assertions resolve to the SAME database.
process.env.EXECUTION_ENV = 'DEV'
process.env.APP_ENV = 'development'

async function createStory(storyId: string): Promise<void> {
  await engineSql()`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'crash window fixture',
      'High', 'Ready', 'temporary', 0, true
    )
    on conflict (id) do nothing
  `
}

/** The engine starts at FORGE_SDLC_VERSION, so the active definition must be present. */
async function ensureForgeDefinition(): Promise<void> {
  const existing = await engineSql()`
    select id from process_definitions
    where key = ${FORGE_SDLC_KEY} and version = ${FORGE_SDLC_VERSION} and tenant_id is null
    limit 1
  `
  if (existing[0]) return
  const graph = parseForgeSdlc().graph
  await engineSql()`
    insert into process_definitions (key, version, name, description, definition, status, created_by)
    values (
      ${FORGE_SDLC_KEY}, ${FORGE_SDLC_VERSION}, 'Forge Software Delivery Lifecycle',
      'crash window fixture seed', ${JSON.stringify(graph)}::jsonb, 'active', 'forge-test'
    )
    on conflict (tenant_id, key, version) do nothing
  `
}

async function cleanup(storyId: string, instanceId: string, receiptIds: string[]): Promise<void> {
  try {
    for (const commandId of receiptIds) {
      await engineSql()`delete from workflow_command_receipt where command_id = ${commandId}`
    }
    if (instanceId) {
      await engineSql()`delete from forge_workflow_evidence where process_instance_id = ${instanceId}`
      await engineSql()`delete from process_events where process_instance_id = ${instanceId}`
      await engineSql()`delete from process_instances where id = ${instanceId}`
    }
    await engineSql()`delete from storyboard_story where id = ${storyId}`
  } catch {
    /* best-effort */
  }
}

test('completion crash window: transition without evidence is completed by the next run exactly once', async () => {
  process.env.APP_ENV = 'development'
  const storyId = `TMP-CRASHWIN-${Date.now()}-${randomUUID().slice(0, 8)}`
  await ensureForgeDefinition()
  await createStory(storyId)
  let instanceId = ''
  const receiptIds: string[] = []
  try {
    const started = await startForgeWorkflow(storyId, { workType: 'FEATURE' })
    instanceId = started.instanceId
    const tasks = await listActiveForgeRoleTasks(storyId)
    assert.ok(tasks.length > 0, 'a fresh FORGE_SDLC instance must park at an async role task')
    const task = tasks[0]
    const actor = task.candidates[0] ?? 'forge'
    await claimForgeRoleTask(task.taskId, actor)
    const receiptId = forgeCompletionReceiptId(task.taskId)
    receiptIds.push(receiptId)

    const evidence: ForgeGateEvidence = { leadDecision: 'SMITH' }

    // INJECT THE FIRST CRASH — the transition wins, the unit never runs.
    await assert.rejects(
      () =>
        completeForgeRoleTask(task.taskId, {
          evidence,
          userId: actor,
          unit: {
            afterTransition: () => {
              throw new Error('injected crash after transition')
            },
          },
        }),
      /injected crash after transition/,
    )

    // Durable state: the workflow ADVANCED, the evidence was NOT written, no receipt.
    const crashed = await readForgeWorkflowEvidence(storyId, engineSql())
    assert.equal(crashed.leadDecision, undefined, 'the crash must leave the evidence unwritten')
    assert.equal(
      await readFinalReceipt(engineSql(), receiptId),
      null,
      'a rolled-back unit must leave no receipt',
    )

    // RESUME: the later run detects the advanced-without-evidence state and COMPLETES it.
    const applied = await reconcileForgeCompletions(storyId)
    assert.equal(applied, 1, 'resume completes exactly the crashed task')
    const recovered = await readForgeWorkflowEvidence(storyId, engineSql())
    assert.equal(recovered.leadDecision, 'SMITH', 'evidence is completed from the durable record')
    assert.ok(
      (await readFinalReceipt(engineSql(), receiptId)) !== null,
      'the completed unit leaves its receipt',
    )

    // EXACTLY ONCE: a second resume writes nothing.
    const again = await reconcileForgeCompletions(storyId)
    assert.equal(again, 0, 'a second resume must not re-apply the unit')

    // The role is NOT re-run: the completed engine task is never offered again.
    const after = await listActiveForgeRoleTasks(storyId)
    assert.ok(
      !after.some((candidate) => candidate.taskId === task.taskId),
      'the completed task must not be offered to the runner again',
    )
  } finally {
    await cleanup(storyId, instanceId, receiptIds)
  }
})

test('completion crash window: repair counter survives a crash between evidence and increment', async () => {
  process.env.APP_ENV = 'development'
  const storyId = `TMP-CRASHWIN-${Date.now()}-${randomUUID().slice(0, 8)}`
  await ensureForgeDefinition()
  await createStory(storyId)
  let instanceId = ''
  const receiptIds: string[] = []
  try {
    const started = await startForgeWorkflow(storyId, { workType: 'FEATURE' })
    instanceId = started.instanceId
    const repairTaskId = randomUUID()
    const receiptId = forgeCompletionReceiptId(repairTaskId)
    receiptIds.push(receiptId)
    const evidence: ForgeGateEvidence = { candidateSha: 'f'.repeat(40) }
    const unit = {
      taskId: repairTaskId,
      processInstanceId: instanceId,
      storyId,
      nodeId: 'repair_smith',
      evidence,
    }

    // INJECT THE SECOND CRASH — the evidence merged, then the counter write failed.
    await assert.rejects(
      () =>
        applyForgeCompletionUnit(unit, {
          afterEvidence: () => {
            throw new Error('injected crash between evidence and increment')
          },
        }),
      /injected crash between evidence and increment/,
    )
    const crashedLedger = await readForgeRepairLedger(storyId, engineSql())
    assert.equal(crashedLedger?.repairAttempts, 0, 'a rolled-back unit must not count the attempt')
    assert.equal(
      await readFinalReceipt(engineSql(), receiptId),
      null,
      'a rolled-back unit must leave no receipt',
    )

    // RESUME: the unit (the primitive the resume reconciliation iterates) completes both
    // effects together, so the attempt can never be left undercounted.
    await applyForgeCompletionUnit(unit)
    const resumed = await readForgeRepairLedger(storyId, engineSql())
    assert.equal(resumed?.repairAttempts, 1, 'the repair attempt is never undercounted')

    // EXACTLY ONCE: another resume must not double-count.
    await applyForgeCompletionUnit(unit)
    const again = await readForgeRepairLedger(storyId, engineSql())
    assert.equal(again?.repairAttempts, 1, 'the receipt keeps the counter exactly-once')
  } finally {
    await cleanup(storyId, instanceId, receiptIds)
  }
})
