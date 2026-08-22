// ---------------------------------------------------------------------------
// ENG-20 Slice B — execution target (control plane vs execution target).
// SCOPED tests only: DEV command resolves DEV execution DB, forced PROD
// mismatch fails fast before external work, execution target is durable on
// command/run/evidence, canonical control-plane persistence still works.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'
import {
  assertExecutionTargetSafe,
  databaseUrlForExecutionTarget,
  isExecutionEnvironment,
  parseExecutionEnvironment,
  resolveExecutionTarget,
  ExecutionTargetError,
} from '../../../lib/execution-target'
import {
  beginAgentWorkRun,
  claimSpecificAgentWork,
  enqueueAgentWorkCommand,
} from '../../../db/agent-work'
import { listStoryRuns } from '../../../db/storyboard'

const executor = interactiveSql as any
let seq = 0

async function createStory(storyId: string): Promise<void> {
  await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes,
      goal, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'ENG-20 execution-target fixture',
      'High', 'Ready', 'temporary target fixture',
      'target goal', 'ENG-20', 'target preconditions',
      'target brief', 'lib/execution-target.ts',
      'target acceptance', 'target postconditions',
      0, true
    )
  `
}

async function cleanupStory(storyId: string): Promise<void> {
  await interactiveSql`delete from storyboard_story where id = ${storyId}`
}

test('ENG-20 target: parse accepts canonical values and aliases; rejects unknown', () => {
  assert.equal(parseExecutionEnvironment('DEV'), 'DEV')
  assert.equal(parseExecutionEnvironment('development'), 'DEV')
  assert.equal(parseExecutionEnvironment('PROD'), 'PROD')
  assert.equal(parseExecutionEnvironment('production'), 'PROD')
  assert.equal(parseExecutionEnvironment('TEST'), 'TEST')
  assert.equal(parseExecutionEnvironment('LOCAL'), 'LOCAL')
  assert.throws(() => parseExecutionEnvironment('sandbox'), ExecutionTargetError)
  assert.throws(() => parseExecutionEnvironment(''), ExecutionTargetError)
  assert.equal(parseExecutionEnvironment('', 'DEV'), 'DEV')
})

test('ENG-20 target: resolve honors explicit EXECUTION_ENV', () => {
  const saved = process.env.EXECUTION_ENV
  process.env.EXECUTION_ENV = 'DEV'
  try {
    assert.equal(resolveExecutionTarget(), 'DEV')
  } finally {
    if (saved === undefined) delete process.env.EXECUTION_ENV
    else process.env.EXECUTION_ENV = saved
  }
})

test('ENG-20 target: DEV command resolves the DEV execution database', () => {
  const saved = process.env.EXECUTION_ENV
  process.env.EXECUTION_ENV = 'DEV'
  try {
    const url = databaseUrlForExecutionTarget('DEV')
    const prodUrl = process.env.DATABASE_URL_PROD ?? null
    assert.ok(url, 'DEV target resolves a database url')
    assert.ok(prodUrl === null || url !== prodUrl, 'DEV target must not resolve to the PROD url')
    assert.ok(isExecutionEnvironment('DEV'))
  } finally {
    if (saved === undefined) delete process.env.EXECUTION_ENV
    else process.env.EXECUTION_ENV = saved
  }
})


test('ENG-20 target: fail-fast — non-PROD target with PROD-resolved DEV url refuses', () => {
  const savedDev = process.env.DATABASE_URL_DEV
  const savedProd = process.env.DATABASE_URL_PROD
  const savedGeneric = process.env.DATABASE_URL
  try {
    process.env.DATABASE_URL_DEV = 'postgres://dev'
    process.env.DATABASE_URL_PROD = 'postgres://prod'
    process.env.DATABASE_URL = 'postgres://dev'
    assert.doesNotThrow(() => assertExecutionTargetSafe('DEV'))

    // Forced PROD mismatch: DATABASE_URL_DEV now points at the PROD database.
    process.env.DATABASE_URL_DEV = 'postgres://prod'
    assert.throws(() => assertExecutionTargetSafe('DEV'), ExecutionTargetError)

    // Generic DATABASE_URL fallback silently pointing at PROD is also refused.
    process.env.DATABASE_URL_DEV = 'postgres://dev'
    process.env.DATABASE_URL = 'postgres://prod'
    assert.throws(() => assertExecutionTargetSafe('DEV'), ExecutionTargetError)

    // PROD target must not resolve to the DEV database.
    assert.throws(() => assertExecutionTargetSafe('PROD'), ExecutionTargetError)
  } finally {
    process.env.DATABASE_URL_DEV = savedDev
    process.env.DATABASE_URL_PROD = savedProd
    process.env.DATABASE_URL = savedGeneric
  }
})

test('ENG-20 target: execution target is durable on command + run evidence', async () => {
  const storyId = `TMP-TARGET-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const item = await enqueueAgentWorkCommand({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      executionPolicy: 'Unattended OK',
      executionEnvironment: 'DEV',
    })
    assert.equal(item.executionEnvironment, 'DEV')

    const claimed = await claimSpecificAgentWork(item.id, 'execution-target-test-worker')
    assert.ok(claimed, 'claim succeeds (no other active item)')

    await beginAgentWorkRun(item.id, executor)
    const runs = await listStoryRuns(storyId, executor)
    assert.ok(runs.length >= 1, 'a run was created')
    assert.equal(runs[0].executionEnvironment, 'DEV', 'run evidence carries the execution target')
  } finally {
    await cleanupStory(storyId)
  }
})

test('ENG-20 target: canonical control-plane persistence still works (policy + claim)', async () => {
  const storyId = `TMP-TARGET-${Date.now()}-${++seq}`
  try {
    await createStory(storyId)
    const item = await enqueueAgentWorkCommand({
      storyId,
      role: 'builder',
      modelProfile: 'builder-flash',
      executionPolicy: 'Human Gate',
      executionEnvironment: 'DEV',
    })
    assert.equal(item.executionPolicy, 'Human Gate')
    assert.equal(item.executionEnvironment, 'DEV')
    const claimed = await claimSpecificAgentWork(item.id, 'execution-target-test-worker')
    assert.ok(claimed)
  } finally {
    await cleanupStory(storyId)
  }
})
