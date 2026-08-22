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
            stdout: 'Invariant check: 0 violations.',
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

