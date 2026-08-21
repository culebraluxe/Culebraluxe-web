// ---------------------------------------------------------------------------
// ENG-18 DOGFOOD DEMONSTRATION — one deterministic local execution through the
// FAKE/TUNIT adapter, end to end:
//
//   1. create a durable work command for a known test fixture story
//   2. poller/invoker claims it
//   3. loads canonical Story Board context
//   4. resolves the TUnit adapter via the registry
//   5. executes deterministic fake work (records heartbeat/progress)
//   6. returns evidence
//   7. writes the associated run evidence
//   8. terminalizes the work item
//   9. verify all three truth layers (storyboard_story / storyboard_story_run
//      / agent_work_item) by querying the DB
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'
import { invokeNextAgentCommand } from '../../../agent-runtime/invoker'
import { AgentRuntimeRegistry } from '../../../agent-runtime/registry'
import { TUnitAgentRuntimeAdapter, type TUnitScenario } from '../../../agent-runtime/tunit-adapter'
import {
  SqlAgentWorkRepository,
  SqlAgentRunRepository,
} from '../../../agent-runtime/repositories'
import { CORE_CAPABILITIES } from '../../../agent-runtime/capabilities'

const executor = async () => interactiveSql as any

let seq = 0

test('ENG-18 dogfood: invoker executes one command via TUnit adapter and all three layers persist', async () => {
  const storyId = `TMP-DOGFOOD-${Date.now()}-${++seq}`
  const work = new SqlAgentWorkRepository(executor)
  const runs = new SqlAgentRunRepository(executor)

  // 1. Known test fixture story (temp, DEV).
  await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes,
      goal, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'ENG-18 dogfood fixture',
      'High', 'Planned', 'temporary dogfood fixture',
      'dogfood goal',
      'ENG-04',
      'dogfood preconditions',
      'DOGFOOD_BRIEF: prove one story maps to one durable command through the invoker.',
      'agent-runtime/*',
      'dogfood: invokeNextAgentCommand returns Complete evidence and persists all layers',
      'dogfood postconditions',
      0, true
    )
  `

  try {
    // 2. Durable work command (role + logical profile + special instructions).
    const item = await work.enqueue({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      specialInstructions: 'dogfood: run the TUnit adapter deterministically',
      priority: 100,
      maxAttempts: 3,
    })

    // Registry: profile -> adapter.
    const registry = new AgentRuntimeRegistry()
    registry.registerAdapter({
      adapterId: 'tunit',
      description: 'deterministic reference adapter',
      capabilities: CORE_CAPABILITIES,
      factory: (deps) =>
        new TUnitAgentRuntimeAdapter(deps, dogfoodScenario()),
    })
    registry.registerProfile({
      profile: 'builder-flash',
      adapterId: 'tunit',
      capabilities: CORE_CAPABILITIES,
    })


    // 2-8. Poller/invoker claims, loads context, resolves adapter, executes,
    // heartbeats, returns evidence, persists run + terminalizes work item.
    const result = await invokeNextAgentCommand('dogfood-worker', {
      work,
      runs,
      registry,
      defaultProfile: 'builder-flash',
    })
    assert.ok(result, 'invoker executed one command')
    assert.equal(result.storyId, storyId)
    assert.equal(result.runtimeAdapter, 'tunit')
    assert.equal(result.modelProfile, 'builder-flash')
    assert.equal(result.evidence.resultStatus, 'Complete')
    assert.equal(result.evidence.completion, 100)

    // 9. Verify all three truth layers by querying the DB.
    //    Layer 1 — storyboard_story: status advanced to Complete.
    const story = await interactiveSql`
      select id, status, completion, actual_start_at, completed_at
      from storyboard_story where id = ${storyId}
    `
    assert.equal((story as any[])[0].status, 'Complete')
    assert.equal((story as any[])[0].completion, 100)
    assert.ok((story as any[])[0].completed_at, 'completed_at set')

    //    Layer 2 — storyboard_story_run: one evidence row with result + notes.
    const runRows = await interactiveSql`
      select id, result_status, completion, notes, tests_summary, ended_at
      from storyboard_story_run where story_id = ${storyId}
    `
    assert.equal((runRows as any[]).length, 1)
    assert.equal((runRows as any[])[0].result_status, 'Complete')
    assert.match((runRows as any[])[0].notes ?? '', /fixture work done/)

    //    Layer 3 — agent_work_item: terminal Done, retained, traceable.
    const workRows = await interactiveSql`
      select id, state, claimed_by, story_run_id, role, model_profile,
        runtime_adapter, external_run_id, attempts, max_attempts
      from agent_work_item where story_id = ${storyId}
    `
    assert.equal((workRows as any[]).length, 1)

function dogfoodScenario(): TUnitScenario {
  return {
    mode: 'success',
    steps: [
      { lifecycle: 'running', note: 'loading_context', completion: 5 },
      { lifecycle: 'running', note: 'executing', completion: 40 },
      { lifecycle: 'running', note: 'running_tests', completion: 70 },
      { lifecycle: 'running', note: 'collecting_evidence', completion: 90 },
    ],
    result: {
      resultStatus: 'Complete',
      completion: 100,
      notes: 'fixture work done via TUnit adapter',
      testsSummary: 'dogfood tests 1/1',
      commitHash: null,
    },
  }
}

    const w = (workRows as any[])[0]
    assert.equal(w.state, 'Done')
    assert.equal(w.claimed_by, 'dogfood-worker')
    assert.ok(w.story_run_id, 'work item linked to the run')
    assert.equal(w.role, 'builder')
    assert.equal(w.model_profile, 'builder-flash')
    assert.equal(w.runtime_adapter, 'tunit')
    assert.match(w.external_run_id ?? '', /^tunit-run-/, 'opaque external correlation stored')
    assert.equal(w.attempts, 0)
    assert.equal(w.max_attempts, 3)
  } finally {
    await interactiveSql`delete from storyboard_story where id = ${storyId}`
  }
})
