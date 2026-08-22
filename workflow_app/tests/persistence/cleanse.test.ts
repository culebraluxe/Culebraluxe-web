// ---------------------------------------------------------------------------
// ENG-20 Slice A — DEV test-data hygiene (surgical preflight cleanse).
// SCOPED tests only (the story's scoped-verification policy): the cleanse
// removes known fixture rows, preserves real stories, removes active fixtures
// safely, and refuses non-DEV invocation. Runs against the DEV database.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'
import { cleanseDevFixtures, isFixtureStoryId } from '../../../db/fixture-cleanup'
import {
  claimSpecificAgentWork,
  enqueueAgentWorkCommand,
} from '../../../db/agent-work'
import { getStoryboardStory } from '../../../db/storyboard'

const executor = interactiveSql as any
let seq = 0

async function createStory(storyId: string, status = 'Planned'): Promise<void> {
  await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes,
      goal, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'ENG-20 cleanse fixture',
      'High', ${status}, 'temporary cleanse fixture',
      'cleanse goal', 'ENG-20', 'cleanse preconditions',
      'cleanse brief', 'db/fixture-cleanup.ts',
      'cleanse acceptance', 'cleanse postconditions',
      0, true
    )
  `
}

async function cleanupStory(storyId: string): Promise<void> {
  await interactiveSql`delete from storyboard_story where id = ${storyId}`
}

test('ENG-20 cleanse: fixture namespace predicate matches only explicit test ids', () => {
  assert.ok(isFixtureStoryId('TMP-POLICY-123'))
  assert.ok(isFixtureStoryId('TUNIT-PROBE'))
  assert.ok(isFixtureStoryId('TEST-SMOKE-X'))
  assert.ok(isFixtureStoryId('ENG-19-DOGFOOD-001'))
  assert.ok(isFixtureStoryId('DOGFOOD-7'))
  assert.ok(!isFixtureStoryId('ENG-20'))
  assert.ok(!isFixtureStoryId('ENG-19'))
  assert.ok(!isFixtureStoryId('CRM-14F'))
  assert.ok(!isFixtureStoryId('ENG-20-SMOKE-001'))
})

test('ENG-20 cleanse: removes known fixture rows and leaves real stories untouched', async () => {
  const fixtureStory = `TMP-CLEANSE-${Date.now()}-${++seq}`
  const keepStory = `KEEP-${Date.now()}-${++seq}`
  try {
    await createStory(fixtureStory)
    await createStory(keepStory)

    // A work item + run for the fixture so dependent rows are exercised.
    const item = await enqueueAgentWorkCommand({
      storyId: fixtureStory,
      role: 'builder',
      modelProfile: 'builder-flash',
      executionPolicy: 'Unattended OK',
      executionEnvironment: 'DEV',
    })
    await claimSpecificAgentWork(item.id, 'cleanse-test-worker')

    const before = await getStoryboardStory(keepStory)
    assert.ok(before, 'keep story exists before cleanse')

    const result = await cleanseDevFixtures({ appEnv: 'development' })
    assert.equal(result.refused, false)
    assert.ok(result.deletedStories.includes(fixtureStory), 'fixture story deleted')
    assert.ok(!result.deletedStories.includes(keepStory), 'real story never deleted')
    assert.ok(result.deletedWorkItems >= 1, 'fixture work item deleted')
    assert.equal(result.activeFixtureCountAfter, 0, 'zero active test-owned work items after cleanup')

    const fixtureGone = await getStoryboardStory(fixtureStory)
    const keepStillThere = await getStoryboardStory(keepStory)
    assert.equal(fixtureGone, null, 'fixture story fully removed')
    assert.ok(keepStillThere, 'real story survives')
  } finally {
    await cleanupStory(fixtureStory)
    await cleanupStory(keepStory)
  }
})

test('ENG-20 cleanse: refuses non-DEV invocation (fail closed)', async () => {
  const prod = await cleanseDevFixtures({ appEnv: 'production' })
  assert.equal(prod.refused, true)
  assert.match(prod.reason ?? '', /DEV\/test only/)

  const staging = await cleanseDevFixtures({ appEnv: 'staging' })
  assert.equal(staging.refused, true)
  assert.match(staging.reason ?? '', /DEV\/test only/)

  const test = await cleanseDevFixtures({ appEnv: 'test' })
  assert.equal(test.refused, false, 'test environment is an allowed DEV-like target')
})

test('ENG-20 cleanse: is idempotent on an already-clean DEV database', async () => {
  const result = await cleanseDevFixtures({ appEnv: 'development' })
  assert.equal(result.refused, false)
  const second = await cleanseDevFixtures({ appEnv: 'development' })
  assert.equal(second.refused, false)
  void executor
})
