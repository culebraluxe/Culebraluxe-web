// ---------------------------------------------------------------------------
// ENG-18 dogfood invoker CLI — deterministic local execution through the
// TUnit/Fake adapter.
// ---------------------------------------------------------------------------

import { interactiveSql } from '../lib/neon-interactive'
import { invokeNextAgentCommand } from '../agent-runtime/invoker'
import { AgentRuntimeRegistry } from '../agent-runtime/registry'
import { TUnitAgentRuntimeAdapter, type TUnitScenario } from '../agent-runtime/tunit-adapter'
import {
  SqlAgentWorkRepository,
  SqlAgentRunRepository,
} from '../agent-runtime/repositories'
import { CORE_CAPABILITIES } from '../agent-runtime/capabilities'

const executor = async () => interactiveSql as any

async function main(): Promise<void> {
  const storyId = `TMP-DOGFOOD-CLI-${Date.now()}`
  const work = new SqlAgentWorkRepository(executor)
  const runs = new SqlAgentRunRepository(executor)

  await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes,
      goal, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'ENG-18 dogfood CLI fixture',
      'High', 'Planned', 'temporary dogfood CLI fixture',
      'dogfood goal', 'ENG-04', 'dogfood preconditions',
      'DOGFOOD_BRIEF: prove the invoker chain end to end.',
      'agent-runtime/*',
      'dogfood: invokeNextAgentCommand returns Complete evidence and persists all layers',
      'dogfood postconditions', 0, true
    )
  `

  try {
    await work.enqueue({
      storyId,
      role: 'builder',
      lane: 'smith',
      runPhase: null,
      modelProfile: 'builder-flash',
      playerId: 'tunit-dogfood',
      providerId: 'forge',
      modelId: 'tunit',
      harnessId: 'forge-native',
      fieldId: 'local',
      specialInstructions: 'dogfood CLI: deterministic TUnit run',
      candidateShas: [],
      priority: 100,
      maxAttempts: 3,
      executionEnvironment: 'DEV',
    })

    const registry = new AgentRuntimeRegistry()
    // The frozen work item says forge-native; for this deterministic diagnostic
    // we deliberately substitute TUnit behind that adapter id. Production team
    // mappings are not involved.
    registry.registerAdapter({
      adapterId: 'deepseek-harness',
      description: 'deterministic reference adapter for dogfood',
      capabilities: CORE_CAPABILITIES,
      factory: (deps) => new TUnitAgentRuntimeAdapter(deps, scenario()),
    })
    registry.registerProfile({
      profile: 'builder-flash',
      adapterId: 'deepseek-harness',
      capabilities: CORE_CAPABILITIES,
    })

    const result = await invokeNextAgentCommand('dogfood-worker', {
      work,
      runs,
      registry,
    })
    if (!result) {
      console.log('no work')
      return
    }

    console.log('=== ENG-18 dogfood result ===')
    console.log('story:', result.storyId)
    console.log('role:', result.role, '| profile:', result.modelProfile, '| adapter:', result.runtimeAdapter)
    console.log('evidence:', result.evidence.resultStatus, result.evidence.completion + '%')

    const [story, runRows, workRows] = await Promise.all([
      interactiveSql`select id, status, completion, completed_at from storyboard_story where id = ${storyId}`,
      interactiveSql`select id, result_status, completion, notes from storyboard_story_run where story_id = ${storyId}`,
      interactiveSql`select id, state, claimed_by, runtime_adapter, external_run_id, role, model_profile from agent_work_item where story_id = ${storyId}`,
    ])
    console.log('--- storyboard_story ---', JSON.stringify(story))
    console.log('--- storyboard_story_run ---', JSON.stringify(runRows))
    console.log('--- agent_work_item ---', JSON.stringify(workRows))
  } finally {
    await interactiveSql`delete from storyboard_story where id = ${storyId}`
  }
}

function scenario(): TUnitScenario {
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
      notes: 'dogfood CLI work done via TUnit adapter',
      testsSummary: 'dogfood tests 1/1',
      commitHash: null,
    },
  }
}

main().catch((e) => {
  console.error(String(e).slice(0, 2000))
  process.exit(1)
})
