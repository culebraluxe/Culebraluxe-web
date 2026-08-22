// ---------------------------------------------------------------------------
// UI-01 — storyboard_story.operating_surface: SCOPED persistence proofs.
//
//   1. operating_surface round-trips create → update → read
//   2. NULL remains valid and explicit (not yet deliberately classified)
//   3. invalid values are rejected by the DB check constraint
// Workstream stays untouched — the surface is a second, independent axis.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'
import type { QueryExecutor } from '../../../db/query-executor'
import {
  createStoryboardStory,
  updateStoryboardStory,
  getStoryboardStory,
  type StoryboardStoryInput,
} from '../../../db/storyboard'

const executor = interactiveSql as unknown as QueryExecutor

const baseId = `UI01-TEST-${Date.now()}`

function input(id: string, operatingSurface: string | null): StoryboardStoryInput {
  return {
    id,
    workstream: 'HARDEN',
    operatingSurface,
    title: 'UI-01 operating_surface persistence fixture',
    priority: 'High',
    status: 'Planned',
    notes: 'temporary UI-01 fixture',
    batch: null,
    goal: null,
    scope: null,
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: null,
    acceptanceCriteria: null,
    postconditions: null,
    completion: 0,
    rollup: false,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
  }
}

async function cleanup(): Promise<void> {
  await interactiveSql`delete from storyboard_story where id like ${`${baseId}%`}`
}

test('UI-01: operating_surface round-trips through the Story Board repository', async () => {
  const id = `${baseId}-RT`
  try {
    const created = await createStoryboardStory(input(id, 'TECH'), executor)
    assert.equal(created.operatingSurface, 'TECH')

    const updated = await updateStoryboardStory(
      id,
      { ...input(id, 'TECH'), operatingSurface: 'NEXUS' },
      executor,
    )
    assert.equal(updated.operatingSurface, 'NEXUS')

    const read = await getStoryboardStory(id, executor)
    assert.equal(read?.operatingSurface, 'NEXUS')
    // The surface is independent from workstream — workstream is untouched.
    assert.equal(read?.workstream, 'HARDEN')
  } finally {
    await cleanup()
  }
})

test('UI-01: NULL operating_surface remains valid and explicit', async () => {
  const id = `${baseId}-NULL`
  try {
    const created = await createStoryboardStory(input(id, null), executor)
    assert.equal(created.operatingSurface, null)
    const read = await getStoryboardStory(id, executor)
    assert.equal(read?.operatingSurface, null, 'null stays null — never a fake surface')
  } finally {
    await cleanup()
  }
})

test('UI-01: invalid operating_surface values are rejected by the check constraint', async () => {
  const id = `${baseId}-BAD`
  try {
    await assert.rejects(
      createStoryboardStory(input(id, 'FRONT_OFFICE'), executor),
      /operating_surface/i,
    )
  } finally {
    await cleanup()
  }
})
