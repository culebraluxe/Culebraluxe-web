// ---------------------------------------------------------------------------
// ENG-20A — queue configuration persistence + hard runtime launch guard.
// TARGETED tests only (the story's scoped-verification policy):
//   1. queue-command persistence (role/profile/instructions/policy/target)
//   2. claim reads the persisted envelope
//   3. missing-config cannot transition to Running (beginRun guard)
//   4. rejectAgentWorkConfiguration fail-fast (Error + Hold + slot released)
//   5. runtime-invocation handoff via invokeNextAgentCommand + TUnit adapter
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'
import {
  beginAgentWorkRun,
  claimNextAgentWork,
  enqueueAgentWorkCommand,
  getAgentWorkItem,
  rejectAgentWorkConfiguration,
  validateAgentWorkLaunchConfig,
} from '../../../db/agent-work'
import { getStoryboardStory, listStoryRuns } from '../../../db/storyboard'
import {
  claimNextAgentCommand,
  executeClaimedAgentCommand,
  invokeNextAgentCommand,
} from '../../../agent-runtime/invoker'
import { AgentRuntimeRegistry } from '../../../agent-runtime/registry'
import { TUnitAgentRuntimeAdapter, type TUnitScenario } from '../../../agent-runtime/tunit-adapter'
import {
  SqlAgentWorkRepository,
  SqlAgentRunRepository,
} from '../../../agent-runtime/repositories'
import { WRITE_CAPABILITIES } from '../../../agent-runtime/lanes'

const executor = interactiveSql as any
let seq = 0

async function createStory(storyId: string, status = 'Planned'): Promise<void> {
  await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes,
      goal, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'ENG-20A launch-guard fixture',
      'High', ${status}, 'temporary launch-guard fixture',
      'launch guard goal', 'ENG-20A', 'launch guard preconditions',
      'LAUNCH_GUARD_BRIEF: prove durable config + pre-Running validation.',
      'agent-runtime/*; db/agent-work.ts',
      'launch guard acceptance', 'launch guard postconditions',
      0, true
    )
  `
}

async function cleanupStory(storyId: string): Promise<void> {
  await interactiveSql`delete from storyboard_story where id = ${storyId}`
}

test('ENG-20A: queueing a command persists the full execution configuration durably', async () => {
  const storyId = `TMP-GUARD-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const item = await enqueueAgentWorkCommand({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      specialInstructions: 'guard: persist me',
      executionPolicy: 'Unattended OK',
      executionEnvironment: 'DEV',
    })
    const stored = await getAgentWorkItem(item.id, executor)
    assert.equal(stored!.role, 'builder')
    assert.equal(stored!.modelProfile, 'builder-flash')
    assert.equal(stored!.specialInstructions, 'guard: persist me')
    assert.equal(stored!.executionPolicy, 'Unattended OK')
    assert.equal(stored!.executionEnvironment, 'DEV')
    assert.equal(stored!.state, 'Ready')
  } finally {
    await cleanupStory(storyId)
  }
})

test('ENG-20A: claim reads the persisted execution envelope (no silent nulls)', async () => {
  const storyId = `TMP-GUARD-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    await enqueueAgentWorkCommand({
      storyId,
      role: 'reviewer',
      modelProfile: 'reviewer',
      executionPolicy: 'Daytime Only',
      executionEnvironment: 'TEST',
    })
    const claim = await claimNextAgentWork('guard-worker')
    assert.ok(claim)
    assert.equal(claim.workItem.role, 'reviewer')
    assert.equal(claim.workItem.modelProfile, 'reviewer')
    assert.equal(claim.workItem.executionPolicy, 'Daytime Only')
    assert.equal(claim.workItem.executionEnvironment, 'TEST')
  } finally {
    await cleanupStory(storyId)
  }
})


test('ENG-20A: missing execution_environment cannot transition a command to Running', async () => {
  const storyId = `TMP-GUARD-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const item = await enqueueAgentWorkCommand({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      // Intentionally NO executionEnvironment — the CRM-14G defect shape.
    })
    const validation = validateAgentWorkLaunchConfig(item)
    assert.match(validation ?? '', /execution_environment/)

    const claimed = await claimNextAgentWork('guard-worker')
    assert.ok(claimed)

    await assert.rejects(
      () => beginAgentWorkRun(item.id, executor),
      (e: Error) => e.message.includes('cannot launch work item') && e.message.includes('execution_environment'),
    )

    const after = await getAgentWorkItem(item.id, executor)
    assert.equal(after!.state, 'Claimed', 'guard fired BEFORE Running')
    const runs = await listStoryRuns(storyId, executor)
    assert.equal(runs.length, 0, 'no fake run was created')
  } finally {
    await cleanupStory(storyId)
  }
})

test('ENG-20A: missing role/profile/target fails fast through the durable path and releases the slot', async () => {
  const storyId = `TMP-GUARD-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const item = await enqueueAgentWorkCommand({ storyId }) // bare command
    const claimed = await claimNextAgentWork('guard-worker')
    assert.ok(claimed)

    const validation = validateAgentWorkLaunchConfig(claimed.workItem)
    assert.ok(validation, 'bare command is invalid')
    const failed = await rejectAgentWorkConfiguration(item.id, validation)

    assert.equal(failed.state, 'Error')
    assert.match(failed.errorText ?? '', /required/)
    const story = await getStoryboardStory(storyId)
    assert.equal(story!.status, 'Hold', 'story set to Hold for human attention')

    // Global slot released: no active item remains system-wide.
    const active = await interactiveSql`select count(*)::int as c from agent_work_item where state in ('Claimed','Running','Paused')`
    assert.equal(active[0].c, 0, 'global execution slot is free')
  } finally {
    await cleanupStory(storyId)
  }
})

test('ENG-20A: correctly configured command reaches the runtime seam with adapter persisted + attempt recorded', async () => {
  const storyId = `TMP-GUARD-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const work = new SqlAgentWorkRepository(() => executor)
    const runs = new SqlAgentRunRepository(() => executor)
    await work.enqueue({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      specialInstructions: 'guard: handoff to the runtime',
      executionPolicy: 'Unattended OK',
      executionEnvironment: 'DEV',
      priority: 100,
      maxAttempts: 2,
    })

    const registry = new AgentRuntimeRegistry()
    registry.registerAdapter({
      adapterId: 'tunit',
      description: 'deterministic reference adapter',
      capabilities: WRITE_CAPABILITIES,
      factory: (deps) => new TUnitAgentRuntimeAdapter(deps, handoffScenario()),
    })
    registry.registerProfile({
      profile: 'builder-flash',
      adapterId: 'tunit',
      capabilities: WRITE_CAPABILITIES,
    })

    const result = await invokeNextAgentCommand('guard-worker', { work, runs, registry })
    assert.ok(result, 'invoker executed the command')
    assert.equal(result.runtimeAdapter, 'tunit')
    assert.equal(result.modelProfile, 'builder-flash')
    assert.equal(result.evidence.resultStatus, 'Complete')

    const item = await getAgentWorkItem(result.workItemId, executor)
    assert.equal(item!.state, 'Done')
    assert.equal(item!.runtimeAdapter, 'tunit', 'runtime adapter persisted durably')
    assert.equal(item!.modelProfile, 'builder-flash', 'profile preserved (no substitution)')
    assert.equal(item!.role, 'builder')
    assert.equal(item!.attempts, 1, 'one attempt recorded at claim')
    assert.equal(item!.executionEnvironment, 'DEV')

    const runRows = await listStoryRuns(storyId, executor)
    assert.equal(runRows.length, 1)
    assert.equal(runRows[0].executionEnvironment, 'DEV')
  } finally {
    await cleanupStory(storyId)
  }

test('ENG-20B: scheduler two-phase dispatch claims then executes the claimed command through the runtime', async () => {
  const storyId = `TMP-GUARD-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const work = new SqlAgentWorkRepository(() => executor)
    const runs = new SqlAgentRunRepository(() => executor)
    await work.enqueue({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      executionPolicy: 'Unattended OK',
      executionEnvironment: 'DEV',
      priority: 100,
      maxAttempts: 2,
    })

    const registry = new AgentRuntimeRegistry()
    registry.registerAdapter({
      adapterId: 'tunit',
      description: 'deterministic reference adapter',
      capabilities: WRITE_CAPABILITIES,
      factory: (deps) => new TUnitAgentRuntimeAdapter(deps, handoffScenario()),
    })
    registry.registerProfile({
      profile: 'builder-flash',
      adapterId: 'tunit',
      capabilities: WRITE_CAPABILITIES,
    })

    // Phase 1 — claim (exactly what the scheduler/poller does).
    const claim = await claimNextAgentCommand('scheduler-worker', { work, runs, registry })
    assert.ok(claim, 'a configured Ready item is claimed atomically')

    // Single-slot invariant: a second claim in the same cycle is refused.
    const second = await claimNextAgentCommand('scheduler-worker', { work, runs, registry })
    assert.equal(second, null, 'no double-claim while an item is active')

    // Phase 2 — execute the ALREADY-CLAIMED command (no story-specific launch).
    const result = await executeClaimedAgentCommand('scheduler-worker', claim, { work, runs, registry })
    assert.equal(result.runtimeAdapter, 'tunit')
    assert.equal(result.modelProfile, 'builder-flash')
    assert.equal(result.evidence.resultStatus, 'Complete')

    const item = await getAgentWorkItem(result.workItemId, executor)
    assert.equal(item!.state, 'Done')
    assert.equal(item!.runtimeAdapter, 'tunit', 'adapter persisted pre-Running')
    assert.equal(item!.attempts, 1)
    assert.equal(item!.executionEnvironment, 'DEV')

    const runRows = await listStoryRuns(storyId, executor)
    assert.equal(runRows.length, 1)
    assert.equal(runRows[0].executionEnvironment, 'DEV')
  } finally {
    await cleanupStory(storyId)
  }
})

test('ENG-20B: invokeNextAgentCommand is the composition of claim + execute (no duplicate mechanism)', async () => {
  const storyId = `TMP-GUARD-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const work = new SqlAgentWorkRepository(() => executor)
    const runs = new SqlAgentRunRepository(() => executor)
    await work.enqueue({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      executionPolicy: 'Unattended OK',
      executionEnvironment: 'DEV',
      priority: 100,
      maxAttempts: 2,
    })

    const registry = new AgentRuntimeRegistry()
    registry.registerAdapter({
      adapterId: 'tunit',
      description: 'deterministic reference adapter',
      capabilities: WRITE_CAPABILITIES,
      factory: (deps) => new TUnitAgentRuntimeAdapter(deps, handoffScenario()),
    })
    registry.registerProfile({
      profile: 'builder-flash',
      adapterId: 'tunit',
      capabilities: WRITE_CAPABILITIES,
    })

    const composed = await invokeNextAgentCommand('debug-worker', { work, runs, registry })
    assert.ok(composed, 'composed poll cycle executes one command')
    assert.equal(composed!.runtimeAdapter, 'tunit')
    assert.equal(composed!.evidence.resultStatus, 'Complete')
    const item = await getAgentWorkItem(composed!.workItemId, executor)
    assert.equal(item!.state, 'Done')
  } finally {
    await cleanupStory(storyId)
  }
})

})

function handoffScenario(): TUnitScenario {
  return {
    mode: 'success',
    steps: [
      { lifecycle: 'running', note: 'executing', completion: 20 },
      { lifecycle: 'running', note: 'running_tests', completion: 60 },
      { lifecycle: 'running', note: 'collecting_evidence', completion: 90 },
    ],
    result: {
      resultStatus: 'Complete',
      completion: 100,
      notes: 'handoff fixture work done',
      testsSummary: 'tunit 1/1',
      commitHash: null,
    },
  }
}
