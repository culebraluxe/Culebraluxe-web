// ---------------------------------------------------------------------------
// SDLC Command Console — execution policy + pause/resume persistence proofs
// against REAL DEV Postgres (migration 029 + db/agent-work.ts).
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'
import {
  enqueueAgentWorkCommand,
  getAgentWorkItem,
  pauseAgentWork,
  resumeAgentWork,
  cancelAgentWork,
  claimSpecificAgentWork,
  beginAgentWorkRun,
} from '../../../db/agent-work'

const executor = interactiveSql as any
let seq = 0

async function createStory(storyId: string): Promise<void> {
  await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes,
      goal, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'ENG-18 console policy fixture',
      'High', 'Ready', 'temporary policy fixture',
      'policy goal', 'ENG-04', 'policy preconditions',
      'policy brief', 'agent-runtime/*',
      'policy acceptance', 'policy postconditions',
      0, true
    )
  `
}

async function cleanupStory(storyId: string): Promise<void> {
  await interactiveSql`delete from storyboard_story where id = ${storyId}`
}

test('console policy: execution policy persists on the durable command envelope', async () => {
  const storyId = `TMP-POLICY-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const item = await enqueueAgentWorkCommand({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      executionPolicy: 'Human Gate',
    })
    assert.equal(item.executionPolicy, 'Human Gate')

    const stored = await getAgentWorkItem(item.id, executor)
    assert.equal(stored!.executionPolicy, 'Human Gate', 'policy persists on the row')
    assert.equal(stored!.state, 'Ready')
  } finally {
    await cleanupStory(storyId)
  }
})

test('console policy: unattended default applies when none provided', async () => {
  const storyId = `TMP-POLICY-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const item = await enqueueAgentWorkCommand({ storyId, modelProfile: 'builder-flash' })
    assert.equal(item.executionPolicy, 'Unattended OK')
  } finally {
    await cleanupStory(storyId)
  }
})

test('console policy: invalid policy is rejected by the DB CHECK constraint', async () => {
  const storyId = `TMP-POLICY-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    await assert.rejects(() =>
      enqueueAgentWorkCommand({ storyId, executionPolicy: 'Whenever' }),
    )
  } finally {
    await cleanupStory(storyId)
  }
})

test('console policy: pause preserves assignment; resume continues the same run', async () => {
  const storyId = `TMP-POLICY-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const item = await enqueueAgentWorkCommand({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      executionEnvironment: 'DEV',
    })
    const claimed = await claimSpecificAgentWork(item.id, 'console-worker')
    assert.ok(claimed)
    const begun = await beginAgentWorkRun(item.id, executor)
    assert.equal(begun.workItem.state, 'Running')
    const runId = begun.workItem.storyRunId!

    const paused = await pauseAgentWork(item.id, executor)
    assert.equal(paused.state, 'Paused')
    assert.equal(paused.claimedBy, 'console-worker', 'assignment preserved across pause')
    assert.equal(paused.storyRunId, runId, 'same run preserved')

    // A second worker must not claim while Paused (single-active includes Paused).
    const second = await claimSpecificAgentWork(item.id, 'other-worker')
    assert.equal(second, null, 'Paused item holds the global slot')

    const resumed = await resumeAgentWork(item.id, executor)
    assert.equal(resumed.state, 'Running')
    assert.equal(resumed.storyRunId, runId, 'resume continues the same logical attempt')

    await cancelAgentWork(item.id, {}, executor)
    const final = await getAgentWorkItem(item.id, executor)
    assert.equal(final!.state, 'Cancelled')
  } finally {
    await cleanupStory(storyId)
  }
})

test('console policy: pause refuses non-Running; resume refuses non-Paused', async () => {
  const storyId = `TMP-POLICY-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const item = await enqueueAgentWorkCommand({ storyId, modelProfile: 'builder-flash' })
    await assert.rejects(() => pauseAgentWork(item.id, executor), /not Running/)
    await assert.rejects(() => resumeAgentWork(item.id, executor), /not Paused/)
  } finally {
    await cleanupStory(storyId)
  }
})
