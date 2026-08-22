// ---------------------------------------------------------------------------
// ENG-19 DeepSeek Harness adapter targeted tests.
//
// These prove the TRANSLATION LAYER behaviors deterministically:
//   - buildTaskText composes canonical Story Board context into the task text
//     without leaking vendor nouns
//   - DeepSeekHarnessAdapter.execute drives the SHARED base lifecycle through a
//     fake harness handle (success + failure + cancel), persists normalized
//     evidence and opaque external correlation via the real repositories
//   - the adapter never emits raw SQL (base owns all persistence)
//
// The fake startRun substitutes the harness process mechanics; the rest of the
// adapter (vendor hooks) is the real production code. Real Postgres (DEV) is
// used for the lifecycle assertions, matching the contract-suite style.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'
import { DeepSeekHarnessAdapter, buildTaskText } from '../../../agent-runtime/deepseek/deepseek-harness-adapter'
import type { DshHandle, DshRunResult } from '../../../agent-runtime/deepseek/dsh-client'
import {
  SqlAgentWorkRepository,
  SqlAgentRunRepository,
} from '../../../agent-runtime/repositories'
import type { AgentExecutionContext, AgentWorkCommand } from '../../../agent-runtime/types'
import type { StoryboardStory } from '../../../db/storyboard'
import type { ChildProcess } from 'node:child_process'
import { provisionWorkerWorkspace } from '../../../lib/worker-workspace'
import { execFile } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

/** Self-contained temp git repo + worktree (never touches the real checkout). */
async function makeTempWorkspace(): Promise<{
  workspace: {
    branchName: string
    worktreePath: string
    baseRef: string
    baseCommit: string
    runId: string
  }
  cleanup: () => Promise<void>
}> {
  const parent = await mkdtemp(join(tmpdir(), 'eng21-ds-'))
  const repoRoot = join(parent, 'repo')
  const worktreesRoot = join(parent, 'worktrees')
  await mkdir(repoRoot, { recursive: true })
  await mkdir(worktreesRoot, { recursive: true })
  await git(repoRoot, ['init', '-b', 'main', '-q'])
  await git(repoRoot, ['config', 'user.email', 'eng21@test'])
  await git(repoRoot, ['config', 'user.name', 'eng21'])
  await writeFile(join(repoRoot, 'README.md'), 'eng21 fixture\n')
  await git(repoRoot, ['add', '.'])
  await git(repoRoot, ['commit', '-m', 'base', '-q'])
  const ws = await provisionWorkerWorkspace({
    storyId: 'eng21ds',
    workerId: 'eng21-ds-worker',
    baseRef: 'main',
    runId: 'run-ds',
    repoRoot,
    worktreesRoot,
  })
  return {
    workspace: {
      branchName: ws.branchName,
      worktreePath: ws.worktreePath,
      baseRef: ws.baseRef,
      baseCommit: ws.baseCommit,
      runId: ws.runId,
    },
    cleanup: async () => {
      await rm(dirname(repoRoot), { recursive: true, force: true })
    },
  }
}

const executor = async () => interactiveSql as any

let seq = 0

async function createFixture() {
  const seqId = ++seq
  const storyId = `TMP-ENG19-DS-${Date.now()}-${seqId}`

  const storyRows = await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes,
      goal, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'ENG-19 DeepSeek fixture',
      'High', 'Planned', 'temporary test fixture',
      'test goal: run one deterministic repo check',
      'ENG-18',
      'fixture preconditions',
      'ARCHITECT_BRIEF_DS: implement a tiny deterministic invariant check.',
      'agent-runtime/*',
      'contract: execute returns Complete evidence via the DeepSeek adapter',
      'fixture postconditions',
      0, true
    )
    returning id, workstream, title, priority, status, notes, batch, goal, scope,
      dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
  `
  const story = storyRows[0] as Record<string, any>
  const work = new SqlAgentWorkRepository(executor)
  const item = await work.enqueue({
    storyId,
    role: 'builder',
    modelProfile: 'builder-flash',
    specialInstructions: 'special: do the fixture work',
    priority: 50,
    maxAttempts: 1,
    executionEnvironment: 'DEV',
  })

  const command: AgentWorkCommand = {
    workItemId: item.id,
    storyId: item.storyId,
    role: 'builder',
    modelProfile: 'builder-flash',
    specialInstructions: 'special: do the fixture work',
    priority: item.priority,
    state: item.state as AgentWorkCommand['state'],
    claimedBy: item.claimedBy,
    claimedAt: item.claimedAt,
    startedAt: item.startedAt,
    finishedAt: item.finishedAt,
    storyRunId: item.storyRunId,
    errorText: item.errorText,
    runtimeAdapter: item.runtimeAdapter,
    externalRunId: item.externalRunId,
    attempts: item.attempts,
    maxAttempts: item.maxAttempts,
    executionEnvironment: item.executionEnvironment,
    createdAt: item.createdAt,
    updatedAt: item.updatedAt,
  }

  return {
    storyId,
    story: story as unknown as StoryboardStory,
    command,
    work,
    cleanup: async () => {
      await interactiveSql`delete from storyboard_story_run where story_id = ${storyId}`
      await interactiveSql`delete from agent_work_item where story_id = ${storyId}`
      await interactiveSql`delete from storyboard_story where id = ${storyId}`
    },
  }
}

function makeContext(command: AgentWorkCommand, story: StoryboardStory): AgentExecutionContext {
  return {
    command,
    story,
    policy: { allowCommit: true, allowDevDbWrite: true, allowControlPlaneWrite: true },
    capabilities: [],
    executionEnvironment: command.executionEnvironment,
    storyRunId: '',
  }
}

function fakeProc(exitCode: number | null): ChildProcess {
  return { exitCode, kill: () => true } as unknown as ChildProcess
}

function fakeHandle(result: DshRunResult, runningThenExit = false): DshHandle {
  let alive = runningThenExit
  const deferred = result as DshRunResult
  let cancelled = false
  return {
    proc: {
      get exitCode() {
        return alive ? null : deferred.exitCode
      },
      kill: () => {
        alive = false
        return true
      },
    } as unknown as ChildProcess,
    promise: Promise.resolve(deferred),
    get cancelled() {
      return cancelled
    },
    pause: () => undefined,
    resume: () => undefined,
    cancel: () => {
      cancelled = true
      alive = false
    },
  }
}

test('ENG-19: buildTaskText composes canonical context without vendor nouns', () => {
  const command: AgentWorkCommand = {
    workItemId: 'wi-1',
    storyId: 'ST',
    role: 'builder',
    modelProfile: 'builder-flash',
    specialInstructions: 'special: extra',
    priority: 0,
    state: 'Ready',
    claimedBy: null,
    claimedAt: null,
    startedAt: null,
    finishedAt: null,
    storyRunId: null,
    errorText: null,
    runtimeAdapter: null,
    externalRunId: null,
    attempts: 0,
    maxAttempts: 3,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
  }
  const story = {
    id: 'ST',
    title: 'A tiny story',
    goal: 'do the thing',
    architectBrief: 'keep it small',
    acceptanceCriteria: 'it works',
  } as unknown as StoryboardStory
  const text = buildTaskText(command, { ...makeContext(command, story) })
  assert.match(text, /Execute SDLC story ST: A tiny story\./)
  assert.match(text, /Goal: do the thing/)
  assert.match(text, /special: extra/)
  assert.match(text, /Acceptance criteria/)
  assert.match(text, /Do NOT push/)
  assert.doesNotMatch(text, /deepseek|DeepSeek|v4|session/i)
})

test('ENG-19: DeepSeekHarnessAdapter success path persists Complete evidence + opaque external id', async () => {
  const fx = await createFixture()
  try {
    const work = fx.work
    const claimed = await work.claimSpecific(fx.command.workItemId, 'eng19-test')
    assert.ok(claimed, 'claim succeeds')

    const adapter = new DeepSeekHarnessAdapter(
      { work, runs: new SqlAgentRunRepository(executor) },
      {
        cliBin: '/fake/dsh/lib/bin.js',
        workspace: process.cwd(),
        startRun: () =>
          fakeHandle({
            status: 'success',
            exitCode: 0,
            stdout: [
              'Invariant check: 0 violations.',
              'Tests: workflow_app/tests/evidence-summary.test.ts 8/8 pass; tsc --noEmit clean',
            ].join('\n'),
            stderr: '',
            sessionId: 'session-11111111-2222-3333-4444-555555555555',
            sessionDir: '/tmp/session-11111111-2222-3333-4444-555555555555',
          }),
      },
    )

    const evidence = await adapter.execute(fx.command, makeContext(fx.command, fx.story)).catch((e) => {
      throw new Error(`execute failed: ${String((e as Error)?.message ?? e)}`)
    })
    assert.equal(evidence.resultStatus, 'Complete')
    assert.equal(evidence.completion, 100)
    assert.match(evidence.notes, /Invariant check/)
    // ENG-08: the concise Tests line in the assistant report becomes the
    // durable tests_summary — not just the harness exit code.
    assert.equal(
      evidence.testsSummary,
      'workflow_app/tests/evidence-summary.test.ts 8/8 pass; tsc --noEmit clean',
    )
    assert.equal(evidence.runtimeAdapter, 'deepseek-harness')
    assert.equal(evidence.externalRunId, 'session-11111111-2222-3333-4444-555555555555')

    const rows = await interactiveSql`
      select state, runtime_adapter, external_run_id from agent_work_item where id = ${fx.command.workItemId}
    `
    assert.equal(rows[0].state, 'Done')
    assert.equal(rows[0].runtime_adapter, 'deepseek-harness')
    assert.equal(rows[0].external_run_id, 'session-11111111-2222-3333-4444-555555555555')

    const runRows = await interactiveSql`
      select result_status, completion, notes from storyboard_story_run where story_id = ${fx.storyId}
    `
    assert.ok(runRows.length >= 1)
    assert.equal(runRows[0].result_status, 'Complete')
  } finally {
    await fx.cleanup()
  }
})

test('ENG-19: DeepSeekHarnessAdapter failure path persists Error terminal state', async () => {
  const fx = await createFixture()
  try {
    const work = fx.work
    const claimed = await work.claimSpecific(fx.command.workItemId, 'eng19-test')
    assert.ok(claimed, 'claim succeeds')

    const adapter = new DeepSeekHarnessAdapter(
      { work, runs: new SqlAgentRunRepository(executor) },
      {
        cliBin: '/fake/dsh/lib/bin.js',
        workspace: process.cwd(),
        startRun: () =>
          fakeHandle({
            status: 'failed',
            exitCode: 1,
            stdout: '',
            stderr: 'dsh: model provider error',
            sessionId: null,
            sessionDir: null,
          }),
      },
    )

    const evidence = await adapter.execute(fx.command, makeContext(fx.command, fx.story))
    assert.equal(evidence.resultStatus, 'Failed')
    assert.match(evidence.notes, /model provider error/)

    const rows = await interactiveSql`
      select state, error_text from agent_work_item where id = ${fx.command.workItemId}
    `
    assert.equal(rows[0].state, 'Error')
    assert.match(rows[0].error_text, /model provider error/)
  } finally {
    await fx.cleanup()
  }
})

test('ENG-08: SCOPED full-regression report flags the violation and replaces the tests summary', async () => {
  const fx = await createFixture()
  try {
    const work = fx.work
    const claimed = await work.claimSpecific(fx.command.workItemId, 'eng19-test')
    assert.ok(claimed, 'claim succeeds')

    const adapter = new DeepSeekHarnessAdapter(
      { work, runs: new SqlAgentRunRepository(executor) },
      {
        cliBin: '/fake/dsh/lib/bin.js',
        workspace: process.cwd(),
        startRun: () =>
          fakeHandle({
            status: 'success',
            exitCode: 0,
            stdout: [
              'I ran pnpm test and it passed.',
              'Tests: full suite 124/124 pass',
            ].join('\n'),
            stderr: '',
            sessionId: null,
            sessionDir: null,
          }),
      },
    )

    const evidence = await adapter.execute(fx.command, makeContext(fx.command, fx.story))
    assert.equal(evidence.resultStatus, 'Complete')
    assert.match(evidence.notes, /TEST-MODE VIOLATION \(SCOPED\)/)
    assert.match(evidence.testsSummary ?? '', /TEST-MODE VIOLATION \(SCOPED\): pnpm test/)
  } finally {
    await fx.cleanup()
  }
})

test('ENG-19: DeepSeekHarnessAdapter cancellation persists Cancelled, never success', async () => {
  const fx = await createFixture()
  try {
    const work = fx.work
    const claimed = await work.claimSpecific(fx.command.workItemId, 'eng19-test')
    assert.ok(claimed, 'claim succeeds')

    // Simulate an EXTERNAL SIGTERM: the harness process is terminated while
    // the run is live. The adapter's statusExternal maps handle.cancelled to
    // the canonical cancelled lifecycle; the shared base terminalizes.
    const handle = fakeHandle(
      {
        status: 'failed',
        exitCode: null,
        stdout: '',
        stderr: 'killed',
        sessionId: null,
        sessionDir: null,
      },
      true,
    )
    const adapter = new DeepSeekHarnessAdapter(
      { work, runs: new SqlAgentRunRepository(executor) },
      {
        cliBin: '/fake/dsh/lib/bin.js',
        workspace: process.cwd(),
        startRun: () => handle,
      },
    )

    const runPromise = adapter.execute(fx.command, makeContext(fx.command, fx.story))
    // Give the execute loop one poll cycle, then terminate externally.
    await new Promise((r) => setTimeout(r, 50))
    handle.cancel()
    const evidence = await runPromise
    assert.equal(evidence.resultStatus, 'Cancelled')

    const rows = await interactiveSql`
      select state from agent_work_item where id = ${fx.command.workItemId}
    `
    assert.equal(rows[0].state, 'Cancelled')
  } finally {
    await fx.cleanup()
  }
})

test('ENG-21: adapter executes inside an isolated worktree and evidence identifies branch/worktree/base commit', async () => {
  const fx = await createFixture()
  const tmp = await makeTempWorkspace()
  try {
    const work = fx.work
    const claimed = await work.claimSpecific(fx.command.workItemId, 'eng21-test')
    assert.ok(claimed, 'claim succeeds')

    // Worker commits inside its isolated worktree (the harness would do this).
    await writeFile(join(tmp.workspace.worktreePath, 'story-work.txt'), 'done\n')
    await git(tmp.workspace.worktreePath, ['add', '.'])
    await git(tmp.workspace.worktreePath, ['commit', '-m', 'ENG-21 story work', '-q'])
    const head = await git(tmp.workspace.worktreePath, ['rev-parse', 'HEAD'])

    const adapter = new DeepSeekHarnessAdapter(
      { work, runs: new SqlAgentRunRepository(executor) },
      {
        cliBin: '/fake/dsh/lib/bin.js',
        workspace: process.cwd(),
        startRun: () =>
          fakeHandle({
            status: 'success',
            exitCode: 0,
            stdout: 'story done.\nTests: worker-workspace.test.ts 8/8 pass; tsc clean',
            stderr: '',
            sessionId: 'session-22222222-2222-3333-4444-555555555555',
            sessionDir: '/tmp/session-22222222-2222-3333-4444-555555555555',
          }),
      },
    )

    const context = {
      ...makeContext(fx.command, fx.story),
      executionWorkspace: tmp.workspace,
    }
    const evidence = await adapter.execute(fx.command, context)
    assert.equal(evidence.resultStatus, 'Complete')
    // Local commit evidence = the worker's own HEAD in the isolated worktree.
    assert.equal(evidence.commitHash, head)
    // The durable narrative identifies branch / worktree / approved base commit.
    assert.match(evidence.notes, /Execution workspace: branch=agent\/eng21ds\/run-ds/)
    assert.match(evidence.notes, new RegExp(`worktree=${tmp.workspace.worktreePath.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`))
    assert.match(evidence.notes, new RegExp(`base=main@${tmp.workspace.baseCommit}`))
    assert.equal(
      evidence.testsSummary,
      'worker-workspace.test.ts 8/8 pass; tsc clean',
    )

    const runRows = await interactiveSql`
      select result_status, commit_hash, notes from storyboard_story_run where story_id = ${fx.storyId}
    `
    assert.equal(runRows[0].commit_hash, head)
    assert.match(runRows[0].notes, /Execution workspace: branch=agent\/eng21ds\/run-ds/)
  } finally {
    await tmp.cleanup()
    await fx.cleanup()
  }
})

test('ENG-21: adapter persists NO worker commit when the isolated worktree is still at the approved base', async () => {
  const fx = await createFixture()
  const tmp = await makeTempWorkspace()
  try {
    const work = fx.work
    const claimed = await work.claimSpecific(fx.command.workItemId, 'eng21-test')
    assert.ok(claimed, 'claim succeeds')

    // The worker left the checkout exactly at the approved base (no commit).
    const adapter = new DeepSeekHarnessAdapter(
      { work, runs: new SqlAgentRunRepository(executor) },
      {
        cliBin: '/fake/dsh/lib/bin.js',
        workspace: process.cwd(),
        startRun: () =>
          fakeHandle({
            status: 'success',
            exitCode: 0,
            stdout: 'inspected only.\nTests: no changes, no tests run',
            stderr: '',
            sessionId: null,
            sessionDir: null,
          }),
      },
    )

    const context = {
      ...makeContext(fx.command, fx.story),
      executionWorkspace: tmp.workspace,
    }
    const evidence = await adapter.execute(fx.command, context)
    assert.equal(evidence.resultStatus, 'Complete')
    // Honest-by-contract: the base commit is NOT persisted as a worker commit.
    assert.equal(evidence.commitHash, null)
    assert.match(evidence.notes, /Execution workspace: branch=agent\/eng21ds\/run-ds/)
  } finally {
    await tmp.cleanup()
    await fx.cleanup()
  }
})

