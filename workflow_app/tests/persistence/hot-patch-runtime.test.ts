// ---------------------------------------------------------------------------
// ENG-20A hot patch — runtime test mode + full-harness guard + heartbeat
// coalescing + in-run DSH session discovery + DEV child-process safety.
// TARGETED tests only (the hot patch's scoped-verification policy): the test
// mode / guard / env-safety behavior is pure; the heartbeat dedup runs against
// the DEV database only.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync } from 'node:fs'
import { tmpdir, homedir } from 'node:os'
import { join } from 'node:path'

import { interactiveSql } from '../../../lib/neon-interactive'
import {
  beginAgentWorkRun,
  claimSpecificAgentWork,
  enqueueAgentWorkCommand,
  updateAgentWorkProgress,
} from '../../../db/agent-work'
import { listStoryRuns } from '../../../db/storyboard'
import {
  resolveTestModeFromInstructions,
  withTestModeDirective,
  detectFullRegressionAttempt,
} from '../../../agent-runtime/test-mode'
import { buildTaskText } from '../../../agent-runtime/deepseek/deepseek-harness-adapter'
import type { AgentWorkCommand, AgentExecutionContext } from '../../../agent-runtime/types'
import {
  buildChildProcessEnv,
  parseEnvFile,
  verifyWorkspaceEnvFile,
  ExecutionTargetError,
} from '../../../lib/execution-target'
import { sessionRootForWorkspace, discoverLatestSession } from '../../../agent-runtime/deepseek/dsh-client'

const executor = interactiveSql as any
let seq = 0

async function createStory(storyId: string, status = 'Planned'): Promise<void> {
  await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes,
      goal, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'ENG-20A hot-patch fixture',
      'High', ${status}, 'temporary hot-patch fixture',
      'hot patch goal', 'ENG-20A', 'hot patch preconditions',
      'HOT_PATCH_BRIEF: full suite + tsc + build',
      'agent-runtime/*',
      'hot patch acceptance', 'hot patch postconditions',
      0, true
    )
  `
}

async function cleanupStory(storyId: string): Promise<void> {
  await interactiveSql`delete from storyboard_story where id = ${storyId}`
}

test('ENG-20A: default test mode is SCOPED; directive is parsed and stripped', () => {
  const noDirective = resolveTestModeFromInstructions('implement the story')
  assert.equal(noDirective.mode, 'SCOPED')
  assert.equal(noDirective.instructions, 'implement the story')

  const withDirective = resolveTestModeFromInstructions(
    withTestModeDirective('verify carefully', 'FULL'),
  )
  assert.equal(withDirective.mode, 'FULL')
  assert.equal(withDirective.instructions, 'verify carefully', 'directive token removed')

  const envOverride = resolveTestModeFromInstructions('do work', 'NONE')
  assert.equal(envOverride.mode, 'NONE')
})

test('ENG-20A: SCOPED task text is authoritative and overrides "full suite" story prose', () => {
  const command = {
    workItemId: 'w',
    storyId: 'CRM-14G',
    role: 'builder',
    modelProfile: 'builder-flash',
    specialInstructions: 'ENG-20 smoke: implement the story exactly as written.',
    priority: 100,
    state: 'Ready' as const,
    claimedBy: null,
    claimedAt: null,
    startedAt: null,
    finishedAt: null,
    storyRunId: null,
    errorText: null,
    runtimeAdapter: null,
    externalRunId: null,
    attempts: 0,
    maxAttempts: 1,
    createdAt: 'x',
    updatedAt: 'x',
  }
  const story = {
    id: 'CRM-14G',
    title: 'Workflow command inventory',
    goal: 'run the full suite + tsc + build',
    architectBrief: 'full suite verification',
    acceptanceCriteria: 'full suite green',
    workstream: 'ENG',
    priority: 'High',
    status: 'Ready',
    notes: '',
    batch: null,
    scope: null,
    dependencies: null,
    preconditions: null,
    contextRefs: null,
    postconditions: null,
    architectBriefUpdatedAt: null,
    completion: 0,
    rollup: true,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    createdAt: 'x',
    updatedAt: 'x',
  }
  const context = { command, story } as unknown as AgentExecutionContext
  const task = buildTaskText(command as unknown as AgentWorkCommand, context)

  assert.match(task, /RUNTIME TEST EXECUTION POLICY: SCOPED/)
  assert.match(task, /FULL regression is NOT authorized/)
  assert.match(task, /pnpm test:persistence/)
  assert.match(task, /OUTRANKS any story prose/)
  assert.ok(!task.includes('[runtime test-mode:'), 'directive token stripped')
  assert.match(task, /full suite \+ tsc \+ build/)
  assert.ok(!/FULL regression is explicitly authorized/.test(task))
})

test('ENG-20A: SCOPED guard detects full-regression aliases, not targeted single-file tests', () => {
  assert.ok(detectFullRegressionAttempt('I ran pnpm test'))
  assert.equal(detectFullRegressionAttempt('I ran pnpm test'), 'pnpm test')
  assert.ok(detectFullRegressionAttempt('verified with pnpm test:persistence'))
  assert.ok(detectFullRegressionAttempt('npm test completed'))
  assert.ok(detectFullRegressionAttempt('pnpm run test:app'))
  assert.ok(detectFullRegressionAttempt('tsx --test workflow_engine/tests/*.test.ts'))
  assert.equal(
    detectFullRegressionAttempt('ran tsx --test workflow_app/tests/persistence/cleanse.test.ts'),
    null,
    'targeted single-file test is allowed in SCOPED mode',
  )
  assert.equal(detectFullRegressionAttempt('ran tsc --noEmit'), null, 'typecheck allowed')
  assert.equal(detectFullRegressionAttempt(null), null)
})


test('ENG-20A: repeated identical heartbeat notes append once; meaningful change appends', async () => {
  const storyId = `TMP-HP-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const item = await enqueueAgentWorkCommand({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      executionPolicy: 'Unattended OK',
      executionEnvironment: 'DEV',
    })
    await claimSpecificAgentWork(item.id, 'hot-patch-worker')
    await beginAgentWorkRun(item.id, executor)

    await updateAgentWorkProgress(item.id, { note: 'deepseek harness running (external session-abc)', completion: 10 }, executor)
    await updateAgentWorkProgress(item.id, { note: 'deepseek harness running (external session-abc)', completion: 20 }, executor)
    await updateAgentWorkProgress(item.id, { note: 'deepseek harness running (external session-abc)' }, executor)
    await updateAgentWorkProgress(item.id, { note: 'tests started' }, executor)

    const runs = await listStoryRuns(storyId, executor)
    const notes = runs[0].notes ?? ''
    const identicalCount = notes.split('\n').filter((l: string) => l.includes('deepseek harness running (external session-abc)')).length
    assert.equal(identicalCount, 1, 'identical heartbeat note appended exactly once')
    assert.match(notes, /tests started/, 'meaningful transition appended')

    const workAfter = await executor`select updated_at from agent_work_item where id = ${item.id}`
    assert.ok(workAfter[0]?.updated_at, 'heartbeat liveness (updated_at) still refreshed')
  } finally {
    await cleanupStory(storyId)
  }
})

test('ENG-20A: discoverLatestSession finds the newest session for the workspace', () => {
  const home = mkdtempSync(join(tmpdir(), 'dsh-hp-'))
  const workspace = '/Users/lisapenfieldicloud.com/Documents/Culebraluxe-web'
  const root = sessionRootForWorkspace(workspace).replace(
    process.env.DSH_HOME ?? join(homedir(), '.dsh'),
    home,
  )
  mkdirSync(join(root, 'session-old'), { recursive: true })
  mkdirSync(join(root, 'session-87aec606-fd5c-4ac3-a545-c81e2b547f0e'), { recursive: true })
  const saved = process.env.DSH_HOME
  process.env.DSH_HOME = home
  try {
    assert.equal(
      discoverLatestSession(workspace),
      'session-87aec606-fd5c-4ac3-a545-c81e2b547f0e',
      'newest session discovered',
    )
  } finally {
    if (saved === undefined) delete process.env.DSH_HOME
    else process.env.DSH_HOME = saved
  }
})



test('ENG-20A: DEV child env forces DEV resolution and removes the PROD url', () => {
  const child = buildChildProcessEnv('DEV')
  assert.equal(child.APP_ENV, 'development')
  assert.equal(child.EXECUTION_ENV, 'DEV')
  assert.equal(child.DATABASE_URL, process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL)
  assert.equal(child.DATABASE_URL_PROD, undefined, 'PROD url removed from DEV child')
})

test('ENG-20A: DEV child env fails fast when DATABASE_URL_DEV resolves to PROD', () => {
  const savedDev = process.env.DATABASE_URL_DEV
  const savedProd = process.env.DATABASE_URL_PROD
  const savedGeneric = process.env.DATABASE_URL
  try {
    process.env.DATABASE_URL_DEV = 'postgres://prod'
    process.env.DATABASE_URL_PROD = 'postgres://prod'
    process.env.DATABASE_URL = 'postgres://dev'
    assert.throws(() => buildChildProcessEnv('DEV'), ExecutionTargetError)
  } finally {
    process.env.DATABASE_URL_DEV = savedDev
    process.env.DATABASE_URL_PROD = savedProd
    process.env.DATABASE_URL = savedGeneric
  }
})

test('ENG-20A: workspace .env.local verification rejects a DEV->PROD resolution', () => {
  const envFile = [
    'APP_ENV=production',
    'DATABASE_URL_DEV=postgres://prod',
    'DATABASE_URL_PROD=postgres://prod',
  ].join('\n')
  const parsed = parseEnvFile(envFile)
  assert.equal(parsed.APP_ENV, 'production')
  assert.equal(parsed.DATABASE_URL_DEV, 'postgres://prod')
  assert.throws(
    () => verifyWorkspaceEnvFile('/fake/workspace', 'DEV', () => envFile),
    ExecutionTargetError,
  )

  const healthy = [
    'APP_ENV=development',
    'DATABASE_URL_DEV=postgres://dev',
    'DATABASE_URL_PROD=postgres://prod',
    'DATABASE_URL=postgres://dev',
  ].join('\n')
  assert.doesNotThrow(() => verifyWorkspaceEnvFile('/fake/workspace', 'DEV', () => healthy))
})
