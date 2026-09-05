// ---------------------------------------------------------------------------
// ENG-18 adapter contract suite run against the deterministic TUnit adapter
// with REAL Postgres persistence (DEV branch). Proves the shared contract:
// lifecycle, idempotency, heartbeat, pause/resume, cancel, terminal guards,
// evidence immutability, restart recovery, cross-story isolation.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'
import {
  runAdapterContractSuite,
  type ContractFixture,
} from '../../../agent-runtime/contract-suite'
import { TUnitAgentRuntimeAdapter, type TUnitScenario } from '../../../agent-runtime/tunit-adapter'
import {
  SqlAgentWorkRepository,
  SqlAgentRunRepository,
  SqlStoryContextRepository,
} from '../../../agent-runtime/repositories'
import type { AgentExecutionContext, AgentWorkCommand } from '../../../agent-runtime/types'
import type { StoryboardStory } from '../../../db/storyboard'

const storyboardTablesReady = async (): Promise<boolean> => {
  const rows = await interactiveSql`select to_regclass('storyboard_story') is not null as s, to_regclass('agent_work_item') is not null as w`
  return rows[0]?.s === true && rows[0]?.w === true
}

const executor = async () => interactiveSql as any

let seq = 0

async function createFixture(): Promise<ContractFixture> {
  if (!(await storyboardTablesReady())) {
    throw new Error('storyboard tables missing from DEV — apply migrations 021-028')
  }
  const seqId = ++seq
  const storyId = `TMP-ENG18-${Date.now()}-${seqId}`

  // Insert a temp story (status Planned so the Ready-dispatch trigger does not
  // fire); enqueue a durable command explicitly.
  const storyRows = await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes,
      goal, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'ENG-18 contract fixture',
      'High', 'Planned', 'temporary test fixture',
      'test goal',
      'ENG-04',
      'fixture preconditions',
      'ARCHITECT_BRIEF_FIXTURE: inspect existing abstractions and reuse them; do not redesign.',
      'agent-runtime/*',
      'contract: execute returns Complete evidence',
      'fixture postconditions',
      0, true
    )
    returning id, workstream, title, priority, status, notes, batch, goal, scope,
      dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
  `
  const story = mapStoryRow(storyRows[0])

  const work = new SqlAgentWorkRepository(executor)
  const item = await work.enqueue({
    storyId,
    role: 'builder',
    modelProfile: 'builder-flash',
    specialInstructions: 'special: do the fixture work',
    priority: 50,
    maxAttempts: 3,
    executionEnvironment: 'DEV',
  })

  const command = toCommandFromItem(item)

  return {
    story,
    command,
    work,
    makeContext: (cmd, st) => makeContext(cmd, st),
    beginRun: async (cmd) => {
      // Claim the command (single-worker-guarded), then begin the run so the
      // work item is Running with a linked story run.
      const claimed = await work.claimSpecific(cmd.workItemId, 'contract')
      if (!claimed) throw new Error(`could not claim ${cmd.workItemId}`)
      const begun = await work.beginRun(cmd.workItemId)
      return toCommandFromItem(begun.workItem)
    },
    cleanup: async () => {
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

function toCommandFromItem(item: {
  id: string
  storyId: string
  state: string
  priority: number
  claimedAt: string | null
  claimedBy: string | null
  startedAt: string | null
  finishedAt: string | null
  storyRunId: string | null
  errorText: string | null
  role: string | null
  modelProfile: string | null
  specialInstructions: string | null
  runtimeAdapter: string | null
  externalRunId: string | null
  attempts: number
  maxAttempts: number
  createdAt: string
  updatedAt: string
}): AgentWorkCommand {
  return {
    workItemId: item.id,
    storyId: item.storyId,
    role: (item.role ?? 'builder') as AgentWorkCommand['role'],
    modelProfile: item.modelProfile ?? 'builder-flash',
    specialInstructions: item.specialInstructions,
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
}

function mapStoryRow(r: any): StoryboardStory {
  return {
    id: r.id,
    workstream: r.workstream,
    title: r.title,
    priority: r.priority,
    status: r.status,
    notes: r.notes,
    batch: r.batch ?? null,
    goal: r.goal ?? null,
    scope: r.scope ?? null,
    dependencies: r.dependencies ?? null,
    preconditions: r.preconditions ?? null,
    architectBrief: r.architect_brief ?? null,
    contextRefs: r.context_refs ?? null,
    acceptanceCriteria: r.acceptance_criteria ?? null,
    postconditions: r.postconditions ?? null,
    architectBriefUpdatedAt: r.architect_brief_updated_at ?? null,
    completion: r.completion,
    rollup: r.rollup,
    plannedStartAt: r.planned_start_at ?? null,
    actualStartAt: r.actual_start_at ?? null,
    completedAt: r.completed_at ?? null,
    createdAt: r.created_at,
    updatedAt: r.updated_at,
  }
}

const successScenario = (): TUnitScenario => ({
  mode: 'success',
  steps: [
    { lifecycle: 'running', note: 'loading_context', completion: 5 },
    { lifecycle: 'running', note: 'executing', completion: 40 },
    { lifecycle: 'running', note: 'running_tests', completion: 70 },
  ],
  result: {
    resultStatus: 'Complete',
    completion: 100,
    notes: 'fixture work done via TUnit adapter',
    testsSummary: 'fixture tests 1/1',
    commitHash: null,
  },
})


runAdapterContractSuite({
  makeAdapter: () =>
    new TUnitAgentRuntimeAdapter(
      {
        work: new SqlAgentWorkRepository(executor),
        runs: new SqlAgentRunRepository(executor),
      },
      successScenario(),
    ),
  makeFixture: createFixture,
})

// Additional ENG-18 persistence proofs beyond the shared suite.
test('ENG-18: TUnit success persists one run + work Done + story status', async () => {
  const f = await createFixture()
  try {
    const adapter = new TUnitAgentRuntimeAdapter(
      { work: f.work, runs: new SqlAgentRunRepository(executor) },
      successScenario(),
    )
    const evidence = await adapter.execute(f.command, f.makeContext(f.command, f.story))
    assert.equal(evidence.resultStatus, 'Complete')
    assert.equal(evidence.completion, 100)

    const item = await f.work.get(f.command.workItemId)
    assert.equal(item!.state, 'Done')

    const runs = await new SqlAgentRunRepository(executor).listForStory(f.story.id)
    assert.equal(runs.length, 1)
    assert.equal(runs[0].resultStatus, 'Complete')
    assert.equal(runs[0].completion, 100)
    assert.match(runs[0].notes ?? '', /fixture work done/)
  } finally {
    await f.cleanup()
  }
})

test('ENG-18: TUnit runtime failure records an Interrupted run and releases the work item for retry (never success)', async () => {
  const f = await createFixture()
  try {
    const adapter = new TUnitAgentRuntimeAdapter(
      { work: f.work, runs: new SqlAgentRunRepository(executor) },
      { mode: 'failure', steps: [{ lifecycle: 'running', note: 'executing', completion: 20 }], error: 'boom' },
    )
    const evidence = await adapter.execute(f.command, f.makeContext(f.command, f.story))
    // Migration 105 interruption model: a runtime failure with retry budget
    // remaining is a RETRYABLE interruption. The run is preserved as
    // Interrupted evidence (never treated as success), and the work item is
    // released back to Ready so a later worker retries the same logical work.
    assert.equal(evidence.resultStatus, 'Interrupted')
    assert.match(evidence.notes, /simulated failure/)

    const item = await f.work.get(f.command.workItemId)
    assert.equal(item!.state, 'Ready', 'released for retry — never terminal Error, never Done')
    assert.equal(item!.errorText, null, 'interruption clears the transient error (not a persisted work Error)')
    assert.ok(item!.storyRunId, 'work item still links the Interrupted run (evidence preserved)')

    const runs = await new SqlAgentRunRepository(executor).listForStory(f.story.id)
    assert.equal(runs.length, 1)
    assert.equal(runs[0].resultStatus, 'Interrupted')
  } finally {
    await f.cleanup()
  }
})

test('ENG-18: TUnit cancel scenario persists Cancelled run + work item (never success)', async () => {
  const f = await createFixture()
  try {
    const adapter = new TUnitAgentRuntimeAdapter(
      { work: f.work, runs: new SqlAgentRunRepository(executor) },
      { mode: 'cancel', steps: [{ lifecycle: 'running', note: 'executing', completion: 20 }], reason: 'operator pause' },
    )
    const evidence = await adapter.execute(f.command, f.makeContext(f.command, f.story))
    assert.equal(evidence.resultStatus, 'Cancelled')

    const item = await f.work.get(f.command.workItemId)
    assert.equal(item!.state, 'Cancelled')
    const runs = await new SqlAgentRunRepository(executor).listForStory(f.story.id)
    assert.equal(runs[0].resultStatus, 'Cancelled')
  } finally {
    await f.cleanup()
  }
})

test('ENG-18: special instructions are additive and canonical architect_brief is never overwritten', async () => {
  const f = await createFixture()
  try {
    const story = f.story
    assert.match(story.architectBrief!, /inspect existing abstractions/)
    assert.ok(!story.architectBrief!.includes('special:'))
    const ctx = new SqlStoryContextRepository(executor)
    const resolved = await ctx.getStory(f.story.id)
    assert.equal(resolved!.architectBrief, story.architectBrief)
    assert.ok(resolved!.acceptanceCriteria!.includes('Complete evidence'))
  } finally {
    await f.cleanup()
  }
})
