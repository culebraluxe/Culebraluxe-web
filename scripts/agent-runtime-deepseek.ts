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

import { invokeNextAgentCommand, buildAgentInvokerWorkspaces } from '../agent-runtime/invoker'
import { createAgentRuntimeRegistry, parseBuilderFlashOverride } from '../agent-runtime/factory'
import type { DeepSeekHarnessConfig } from '../agent-runtime/deepseek/deepseek-harness-adapter'
import {
  SqlAgentWorkRepository,
  SqlAgentRunRepository,
} from '../agent-runtime/repositories'
import { interactiveSql } from '../lib/neon-interactive'
import {
  assertExecutionTargetSafe,
  parseExecutionEnvironment,
  verifyWorkspaceEnvFile,
} from '../lib/execution-target'
import { enqueueAgentWorkCommand, escalateAgentWorkFailure } from '../db/agent-work'
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
  // DEV-safety: verify the workspace .env.local (the config any spawned test
  // process would read) cannot resolve a DEV execution to the PROD application
  // database. The sanitized child env additionally strips DATABASE_URL_PROD.
  verifyWorkspaceEnvFile(WORKSPACE, target)

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

  // ENG-FORGE-V5-03: `builder-flash` now DEFAULTS to provider opencode /
  // adapter opencode-harness. This driver is the EXPLICIT DeepSeek-harness
  // dogfood path, so it pins the Smith profile back to the forge-native
  // DeepSeek harness unless the operator already selected a provider override.
  // Factory finding #1: parse the override once at the call boundary and pass
  // it explicitly; the factory no longer reads process.env mid-loop.
  const builderFlashOverride =
    parseBuilderFlashOverride(process.env.FORGE_PROVIDER_BUILDER_FLASH ?? null) ??
    (process.env.FORGE_EXECUTION_PROVIDER?.trim() ? undefined : 'deepseek' as const)

  const registry = createAgentRuntimeRegistry({
    deepseek: deepseekConfig(),
    ...(builderFlashOverride ? { builderFlashOverride } : {}),
  })

  // ENG-21 — isolated worker workspace execution (same default-on rule as the
  // scheduled worker). The interactive architecture agent executes in its OWN
  // branch + worktree from the EXPLICIT approved integration base; the primary
  // checkout is never a worker scratch directory.
  const workspaces = buildAgentInvokerWorkspaces(workerId)
  if (workspaces) {
    console.log(
      'workspace execution: base ref',
      workspaces.baseRef,
      '| worktrees root:',
      workspaces.worktreesRoot ?? 'default (../Culebraluxe-worktrees)',
    )
  } else {
    console.log('workspace execution: DISABLED (legacy shared-checkout path)')
  }

  let result
  try {
    result = await invokeNextAgentCommand(workerId, {
      work,
      runs,
      registry,
      ...(workspaces ? { workspaces } : {}),
    })
  } catch (e) {
    // ESCALATION (ENG-20/ENG-20B fail-fast policy): mark the work item Error
    // with concise evidence instead of leaving it active (slot released).
    console.error('driver escalation:', String((e as Error)?.message ?? e).slice(0, 2000))
    await escalateAgentWorkFailure(item.id, e, 'ESCALATION: what failed: dispatch via deepseek-harness | what was tried: claim + execute | likely root cause: execution guard or harness start failure | recommended human action: review the error and run notes, then set the story back to Ready and retry.')
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

main().catch((e) => {
  console.error(String(e).slice(0, 3000))
  process.exit(1)
})

