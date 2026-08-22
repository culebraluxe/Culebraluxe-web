// ---------------------------------------------------------------------------
// ENG-19/ENG-20 DeepSeek dogfood/smoke driver — the REAL durable path:
//   Story Board -> agent_work_item -> invoker -> AgentRuntimeAdapter ->
//   DeepSeekHarnessAdapter -> DeepSeek Harness headless CLI -> local repo work
//   -> normalized evidence -> storyboard_story_run -> agent_work_item terminal
//   -> Story Board outcome.
//
// Usage:
//   pnpm agent:runtime:deepseek <storyId> [workerId] [instructions]
//
//   APP_ENV         controls the CONTROL PLANE (production -> DATABASE_URL_PROD;
//                   unset/development -> DATABASE_URL_DEV).
//   EXECUTION_ENV   controls the EXECUTION TARGET (DEV|PROD|TEST|LOCAL,
//                   default DEV). Before any work begins the application/domain
//                   DB configuration is verified against this target and a
//                   non-PROD target that would resolve to the production
//                   application DB is refused (fail-fast, ENG-20).
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
//
// ESCALATION (ENG-20 policy): if the claim was made but the run fails or the
// driver cannot complete, the work item is marked Error with concise escalation
// evidence (what failed / what was tried / likely root cause / blocker /
// recommended human action) instead of being left active or falsely Complete.
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
import {
  assertExecutionTargetSafe,
  parseExecutionEnvironment,
} from '../lib/execution-target'
import { enqueueAgentWorkCommand, failAgentWork } from '../db/agent-work'
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
  const workerId = process.argv[3] ?? 'eng20-smoke-worker'
  const instructions = process.argv[4] ??
    'ENG-20 smoke: implement the story exactly as written. Verify, then commit locally. Never push.'

  // Resolve the INTENDED execution target (explicit EXECUTION_ENV, default DEV
  // for the SDLC DeepSeek path) and FAIL-FAST if the application/domain DB
  // configuration would resolve a non-PROD target to the production database.
  const target = parseExecutionEnvironment(process.env.EXECUTION_ENV, 'DEV')
  console.log('execution target:', target, '| control plane: APP_ENV=' + (process.env.APP_ENV ?? 'development'))
  assertExecutionTargetSafe(target)

  const work = new SqlAgentWorkRepository(() => interactiveSql as any)
  const runs = new SqlAgentRunRepository(() => interactiveSql as any)

  // Queue the durable command through the console's repository seam.
  const item = await enqueueAgentWorkCommand({
    storyId,
    role: 'builder',
    modelProfile: 'builder-flash',
    specialInstructions: instructions,
    priority: 100,
    maxAttempts: 1,
    executionPolicy: 'Unattended OK',
    executionEnvironment: target,
  })
  console.log('queued work item', item.id, 'state=' + item.state, 'policy=' + item.executionPolicy, 'target=' + item.executionEnvironment)

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

  let result
  try {
    result = await invokeNextAgentCommand(workerId, {
      work,
      runs,
      registry,
      defaultProfile: 'builder-flash',
    })
  } catch (e) {
    // ESCALATION (ENG-20 fail-fast policy): mark the work item Error with
    // concise evidence instead of leaving it active. The claim was made, so
    // the run (if begun) is terminalized Failed by failAgentWork.
    const msg = String((e as Error)?.message ?? e).slice(0, 2000)
    console.error('driver escalation:', msg)
    const stored = await work.get(item.id)
    if (stored && stored.state !== 'Done' && stored.state !== 'Error' && stored.state !== 'Cancelled') {
      try {
        await failAgentWork(item.id, msg, {
          note: 'ESCALATION: what failed: ' + msg.slice(0, 300) + ' | what was tried: claim + execute via deepseek-harness | likely root cause: execution guard or harness start failure | recommended human action: review DATABASE_URL_DEV/PROD config and run notes, then set the story back to Ready and retry.',
        })
      } catch (e2) {
        console.error('escalation record failed:', String((e2 as Error)?.message ?? e2))
      }
    }
    process.exit(1)
  }
  if (!result) {
    console.error('no work was claimed for ' + storyId)
    process.exit(1)
  }

  console.log('=== DeepSeek dogfood/smoke result ===')
  console.log('story:', result.storyId)
  console.log('role:', result.role, '| profile:', result.modelProfile, '| adapter:', result.runtimeAdapter)
  console.log('evidence:', result.evidence.resultStatus, result.evidence.completion + '%')
  console.log('external_run_id:', result.evidence.externalRunId)
  console.log('execution_target:', result.evidence.executionEnvironment ?? 'unset')

  const [story, runRows, workRows] = await Promise.all([
    interactiveSql`select id, status, completion, completed_at from storyboard_story where id = ${storyId}`,
    interactiveSql`select id, result_status, completion, commit_hash, execution_environment, notes from storyboard_story_run where story_id = ${storyId} order by created_at desc limit 3`,
    interactiveSql`select id, state, claimed_by, runtime_adapter, external_run_id, role, model_profile, execution_policy, execution_environment, attempts from agent_work_item where story_id = ${storyId}`,
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

