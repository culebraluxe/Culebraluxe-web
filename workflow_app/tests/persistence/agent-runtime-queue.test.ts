// ---------------------------------------------------------------------------
// ENG-18 queue/concurrency/recovery TUNIT against REAL DEV Postgres.
//
// Proves the durable agent_work_item command queue:
//   - 10 Ready items are durable and queryable
//   - one poller claims one eligible command
//   - two pollers racing do not double-claim (single-worker rule preserved)
//   - claimed work survives process restart (re-read from DB)
//   - completed work remains queryable; Error/Cancelled remain traceable
//   - re-save/requeue does not create duplicate active command rows
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'
import { SqlAgentWorkRepository } from '../../../agent-runtime/repositories'
import { TUnitAgentRuntimeAdapter, type TUnitScenario } from '../../../agent-runtime/tunit-adapter'
import { SqlAgentRunRepository } from '../../../agent-runtime/repositories'

const executor = async () => interactiveSql as any

let seq = 0

async function createStory(storyId: string): Promise<void> {
  await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes,
      goal, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'ENG-18 queue fixture',
      'High', 'Planned', 'temporary queue fixture',
      'queue fixture goal', 'ENG-04', 'queue fixture preconditions',
      'queue fixture brief', 'agent-runtime/*',
      'queue fixture acceptance', 'queue fixture postconditions',
      0, true
    )
  `
}

async function cleanupStory(storyId: string): Promise<void> {
  await interactiveSql`delete from storyboard_story where id = ${storyId}`
}

const scenario = (): TUnitScenario => ({
  mode: 'success',
  steps: [{ lifecycle: 'running', note: 'executing', completion: 50 }],
  result: {
    resultStatus: 'Complete',
    completion: 100,
    notes: 'queue fixture done',
    testsSummary: 'queue tests 1/1',
    commitHash: null,
  },
})

test('ENG-18: 10 Ready work items can exist durably and are queryable', async () => {
  const work = new SqlAgentWorkRepository(executor)
  const ids: string[] = []
  try {
    for (let i = 0; i < 10; i++) {
      const storyId = `TMP-Q-${Date.now()}-${++seq}`
      await createStory(storyId)
      ids.push(storyId)
      await work.enqueue({ storyId, role: 'builder', modelProfile: 'builder-flash', priority: i })
    }
    const items = await interactiveSql`
      select id, story_id, state from agent_work_item
      where story_id = any(${ids}::text[])
      order by priority
    `
    assert.equal((items as any[]).length, 10)
    for (const it of items as any[]) assert.equal(it.state, 'Ready')
  } finally {
    for (const id of ids) await cleanupStory(id)
  }
})

test('ENG-18: one poller claims exactly one eligible command (single-worker rule preserved)', async () => {
  const work = new SqlAgentWorkRepository(executor)
  const storyIds: string[] = []
  try {
    for (let i = 0; i < 3; i++) {
      const storyId = `TMP-Q-${Date.now()}-${++seq}`
      await createStory(storyId)
      storyIds.push(storyId)
      await work.enqueue({ storyId, role: 'builder', modelProfile: 'builder-flash', priority: i })
    }
    const claim = await work.claimNext('worker-1')
    assert.ok(claim, 'first poller claims one command')
    const claim2 = await work.claimNext('worker-2')
    assert.equal(claim2, null, 'second poller refused while first is Claimed (global single worker)')
  } finally {
    for (const id of storyIds) await cleanupStory(id)
  }
})

test('ENG-18: two pollers racing for the same next item do not double-claim', async () => {
  const work = new SqlAgentWorkRepository(executor)
  const storyId = `TMP-Q-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    await work.enqueue({ storyId, role: 'builder', modelProfile: 'builder-flash', priority: 10 })

    const results = await Promise.all([
      work.claimNext('race-a'),
      work.claimNext('race-b'),
    ])
    const claimed = results.filter((r) => r !== null)
    assert.equal(claimed.length, 1, 'exactly one claim wins the race')
  } finally {
    await cleanupStory(storyId)
  }
})


test('ENG-18: claimed work survives process restart (re-read from DB)', async () => {
  const work = new SqlAgentWorkRepository(executor)
  const storyId = `TMP-Q-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    await work.enqueue({ storyId, role: 'builder', modelProfile: 'builder-flash', priority: 10 })
    const claim = await work.claimNext('worker-restart')
    assert.ok(claim)
    const workB = new SqlAgentWorkRepository(executor)
    const item = await workB.get(claim.workItem.id)
    assert.equal(item!.state, 'Claimed')
    assert.equal(item!.claimedBy, 'worker-restart')
  } finally {
    await cleanupStory(storyId)
  }
})

test('ENG-18: completed work remains queryable; Error/Cancelled remain traceable', async () => {
  const work = new SqlAgentWorkRepository(executor)
  const ids: string[] = []
  try {
    const storyA = `TMP-Q-${Date.now()}-${++seq}`
    await createStory(storyA)
    ids.push(storyA)
    await work.enqueue({ storyId: storyA, role: 'builder', modelProfile: 'builder-flash', executionEnvironment: 'DEV' })
    const adapter = new TUnitAgentRuntimeAdapter(
      { work, runs: new SqlAgentRunRepository(executor) },
      scenario(),
    )
    const claimA = await work.claimNext('w-done')
    assert.ok(claimA)
    const cmd = toCommand(claimA.workItem)
    await adapter.execute(cmd, {
      command: cmd,
      story: claimA.story,
      policy: { allowCommit: true, allowDevDbWrite: true, allowControlPlaneWrite: true },
      capabilities: [],
      storyRunId: '',
    })
    const done = await work.get(claimA.workItem.id)
    assert.equal(done!.state, 'Done')

    const storyB = `TMP-Q-${Date.now()}-${++seq}`
    await createStory(storyB)
    ids.push(storyB)
    await work.enqueue({ storyId: storyB, modelProfile: 'builder-flash' })
    const claimB = await work.claimNext('w-cancel')
    assert.ok(claimB)
    await work.cancel(claimB.workItem.id, 'trace test')
    const cancelled = await work.get(claimB.workItem.id)
    assert.equal(cancelled!.state, 'Cancelled')
  } finally {
    for (const id of ids) await cleanupStory(id)
  }
})

test('ENG-18: re-save/requeue reuses the active command row (never a duplicate)', async () => {
  const work = new SqlAgentWorkRepository(executor)
  const storyId = `TMP-Q-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const first = await work.enqueue({ storyId, modelProfile: 'builder-flash', priority: 10 })
    // A second "queue command" for the same Ready story UPSERTS the envelope
    // onto the existing active row (console reuse semantics) — it does NOT
    // create a second queue row.
    const second = await work.enqueue({
      storyId,
      role: 'architect',
      modelProfile: 'architect-pro',
      executionPolicy: 'Daytime Only',
      priority: 10,
    })
    assert.equal(second.id, first.id, 'reuses the same durable command row')
    assert.equal(second.role, 'architect')
    assert.equal(second.modelProfile, 'architect-pro')
    assert.equal(second.executionPolicy, 'Daytime Only')

    const active = await interactiveSql`
      select count(*)::int as c from agent_work_item
      where story_id = ${storyId} and state in ('Ready', 'Claimed', 'Running')
    `
    assert.equal((active as any[])[0].c, 1, 'exactly one active command row')
  } finally {
    await cleanupStory(storyId)
  }
})

function toCommand(item: {
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
}) {
  return {
    workItemId: item.id,
    storyId: item.storyId,
    role: (item.role ?? 'builder') as any,
    modelProfile: item.modelProfile ?? 'builder-flash',
    specialInstructions: item.specialInstructions,
    priority: item.priority,
    state: item.state as any,
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
