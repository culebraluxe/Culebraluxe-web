// ---------------------------------------------------------------------------
// ENG-19 DeepSeek dogfood driver — the REAL durable path:
//   Story Board -> agent_work_item -> invoker -> AgentRuntimeAdapter ->
//   DeepSeekHarnessAdapter -> DeepSeek Harness headless CLI -> local repo work
//   -> normalized evidence -> storyboard_story_run -> agent_work_item terminal
//   -> Story Board outcome.
//
// Usage:
//   pnpm agent:runtime:deepseek <storyId>
//
// The command is queued through enqueueAgentWorkCommand — the EXACT repository
// seam the SDLC Command Console server action uses (queueCommandAction). Browser
// interaction is intentionally bypassed so the autonomous proof is not
// contaminated; this is the same durable row the console would have written.
//
// The invoker (invokeNextAgentCommand) claims the item, loads canonical Story
// Board context by story_id, resolves the DeepSeek harness adapter from the
// logical model profile, and drives the shared lifecycle in AgentRuntimeAdapter.
//
// Nothing here contains DeepSeek model ids or session ids. The adapter owns all
// provider resolution below the boundary.
// ---------------------------------------------------------------------------

import { invokeNextAgentCommand } from '../agent-runtime/invoker'
import { AgentRuntimeRegistry } from '../agent-runtime/registry'
import { TUnitAgentRuntimeAdapter, type TUnitScenario } from '../agent-runtime/tunit-adapter'
import { DeepSeekHarnessAdapter, type DeepSeekHarnessConfig } from '../agent-runtime/deepseek/deepseek-harness-adapter'
import {
  SqlAgentWorkRepository,
  SqlAgentRunRepository,
} from '../agent-runtime/repositories'
import { CORE_CAPABILITIES } from '../agent-runtime/capabilities'
import { interactiveSql } from '../lib/neon-interactive'
import { enqueueAgentWorkCommand } from '../db/agent-work'
import { homedir } from 'node:os'
import { join, resolve } from 'node:path'

const WORKSPACE = resolve(process.cwd())
const DSH_BIN = process.env.DSH_CLI_BIN ?? join(homedir(), '.dsh', 'profiles', 'node_modules', '@deepseek-ai', 'dsh', 'lib', 'bin.js')

function deepseekConfig(): DeepSeekHarnessConfig {
  return {
    cliBin: DSH_BIN,
    workspace: WORKSPACE,
  }
}

async function main(): Promise<void> {
  const storyId = process.argv[2] ?? 'ENG-19-DOGFOOD-001'

  const work = new SqlAgentWorkRepository(() => interactiveSql as any)
  const runs = new SqlAgentRunRepository(() => interactiveSql as any)

  // Queue the durable command through the console's repository seam.
  const item = await enqueueAgentWorkCommand({
    storyId,
    role: 'builder',
    modelProfile: 'builder-flash',
    specialInstructions: 'ENG-19 dogfood: run the real DeepSeek Harness headless CLI. Implement the story, verify, and commit locally. Never push.',
    priority: 100,
    maxAttempts: 1,
    executionPolicy: 'Unattended OK',
  })
  console.log('queued work item', item.id, 'state=' + item.state, 'policy=' + item.executionPolicy)

  const registry = new AgentRuntimeRegistry()
  registry.registerAdapter({
    adapterId: 'tunit',
    description: 'deterministic reference adapter',
    capabilities: CORE_CAPABILITIES,
    factory: (deps) => new TUnitAgentRuntimeAdapter(deps, scenario()),
  })
  registry.registerAdapter({
    adapterId: 'deepseek-harness',
    description: 'DeepSeek Harness headless CLI adapter',
    capabilities: CORE_CAPABILITIES,
    factory: (deps) => new DeepSeekHarnessAdapter(deps, deepseekConfig()),
  })
  // Logical profiles resolve below the boundary; the generic command only
  // carries the LOGICAL profile id.
  registry.registerProfile({
    profile: 'builder-flash',
    adapterId: 'deepseek-harness',
    capabilities: CORE_CAPABILITIES,
  })

  const result = await invokeNextAgentCommand('eng19-dogfood-worker', {
    work,
    runs,
    registry,
    defaultProfile: 'builder-flash',
  })
  if (!result) {
    console.error('no work was claimed for ' + storyId)
    process.exit(1)
  }

  console.log('=== ENG-19 DeepSeek dogfood result ===')
  console.log('story:', result.storyId)
  console.log('role:', result.role, '| profile:', result.modelProfile, '| adapter:', result.runtimeAdapter)
  console.log('evidence:', result.evidence.resultStatus, result.evidence.completion + '%')
  console.log('external_run_id:', result.evidence.externalRunId)

  const [story, runRows, workRows] = await Promise.all([
    interactiveSql`select id, status, completion, completed_at from storyboard_story where id = ${storyId}`,
    interactiveSql`select id, result_status, completion, commit_hash, external_run_id, notes from storyboard_story_run where story_id = ${storyId}`,
    interactiveSql`select id, state, claimed_by, runtime_adapter, external_run_id, role, model_profile, execution_policy, attempts from agent_work_item where story_id = ${storyId}`,
  ])
  console.log('--- storyboard_story ---', JSON.stringify(story))
  console.log('--- storyboard_story_run ---', JSON.stringify(runRows))
  console.log('--- agent_work_item ---', JSON.stringify(workRows))
}

function scenario(): TUnitScenario {
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
      notes: 'tunit fallback (not used when deepseek profile is active)',
      testsSummary: 'tunit 1/1',
      commitHash: null,
    },
  }
}

main().catch((e) => {
  console.error(String(e).slice(0, 3000))
  process.exit(1)
})

