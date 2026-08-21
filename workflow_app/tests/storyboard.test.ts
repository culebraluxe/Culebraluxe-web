import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  createStoryboardStory,
  isStoryboardTableReady,
  listStoryboardStories,
  setStoryboardStatus,
  updateStoryboardStory,
} from '../../db/storyboard'
import { PortalWriteError } from '../../lib/portal-write-error'
import type { QueryExecutor } from '../../db/query-executor'

// Minimal in-memory fake for the storyboard_story surface used by
// db/storyboard.ts. No database, no packages.

type Row = Record<string, any>

class FakeDb {
  rows: Row[] = []
  tableReady = true

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''),
        '',
      ),
    )
    const p = params as any[]

    if (t.includes('to_regclass')) {
      return Promise.resolve([{ ready: this.tableReady }])
    }

    if (t.includes('insert into storyboard_story')) {
      if (this.rows.some((r) => r.id === p[0])) return Promise.resolve([])
      const row = {
        id: p[0],
        workstream: p[1],
        title: p[2],
        priority: p[3],
        status: p[4],
        notes: p[5],
        batch: p[6] ?? null,
        goal: p[7] ?? null,
        scope: p[8] ?? null,
        acceptance_criteria: p[9] ?? null,
        dependencies: p[10] ?? null,
        created_at: '2026-08-21T00:00:00Z',
        updated_at: '2026-08-21T00:00:00Z',
      }
      this.rows.push(row)
      return Promise.resolve([row])
    }

    if (t.includes('update storyboard_story')) {
      const statusOnly =
        t.includes('set status = $1') && !t.includes('set workstream')
      const id = statusOnly ? p[1] : p[10]
      const r = this.rows.find((x) => x.id === id)
      if (!r) return Promise.resolve([])
      if (statusOnly) {
        r.status = p[0]
      } else {
        r.workstream = p[0]
        r.title = p[1]
        r.priority = p[2]
        r.status = p[3]
        r.notes = p[4]
        r.batch = p[5] ?? null
        r.goal = p[6] ?? null
        r.scope = p[7] ?? null
        r.acceptance_criteria = p[8] ?? null
        r.dependencies = p[9] ?? null
      }
      r.updated_at = '2026-08-21T01:00:00Z'
      return Promise.resolve([r])
    }

    if (t.includes('from storyboard_story')) {
      return Promise.resolve(this.rows)
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }
}

const baseInput = {
  id: 'CRM-19',
  workstream: 'CRM / Intake',
  title: 'WhatsApp live connector',
  priority: 'High',
  status: 'Open',
  notes: 'Provider webhook + signature verification.',
  batch: null,
  goal: null,
  scope: null,
  acceptanceCriteria: null,
  dependencies: null,
}

test('isStoryboardTableReady reports the probe result', async () => {
  const f = new FakeDb()
  assert.equal(await isStoryboardTableReady(f.tx), true)
  f.tableReady = false
  assert.equal(await isStoryboardTableReady(f.tx), false)
})

test('listStoryboardStories returns null when the table is missing', async () => {
  const f = new FakeDb()
  f.tableReady = false
  assert.equal(await listStoryboardStories(f.tx), null)
})

test('createStoryboardStory inserts and returns the camelCase record', async () => {
  const f = new FakeDb()
  const story = await createStoryboardStory(baseInput, f.tx)
  assert.equal(story.id, 'CRM-19')
  assert.equal(story.workstream, 'CRM / Intake')
  assert.equal(story.title, 'WhatsApp live connector')
  assert.equal(story.priority, 'High')
  assert.equal(story.status, 'Open')
  assert.equal(story.notes, 'Provider webhook + signature verification.')
  assert.equal(story.acceptanceCriteria, null)
  assert.equal(f.rows.length, 1)
})

test('createStoryboardStory with a duplicate id returns a conflict error', async () => {
  const f = new FakeDb()
  await createStoryboardStory(baseInput, f.tx)
  await assert.rejects(
    createStoryboardStory(baseInput, f.tx),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'conflict' &&
      /already exists/i.test(error.message),
  )
})

test('updateStoryboardStory updates fields and preserves the id', async () => {
  const f = new FakeDb()
  await createStoryboardStory(baseInput, f.tx)
  const updated = await updateStoryboardStory(
    'CRM-19',
    {
      workstream: 'Platform / Engineering / Data',
      title: 'WhatsApp connector (renamed)',
      priority: 'Critical',
      status: 'Blocked',
      notes: 'Needs the S-008 channel decision.',
      batch: 4,
      goal: 'Lower provider deliveries.',
      scope: 'Webhook only.',
      acceptanceCriteria: 'Idempotent receipt.',
      dependencies: 'S-008',
    },
    f.tx,
  )
  assert.equal(updated.id, 'CRM-19')
  assert.equal(updated.workstream, 'Platform / Engineering / Data')
  assert.equal(updated.title, 'WhatsApp connector (renamed)')
  assert.equal(updated.priority, 'Critical')
  assert.equal(updated.status, 'Blocked')
  assert.equal(updated.batch, 4)
  assert.equal(updated.goal, 'Lower provider deliveries.')
  assert.equal(updated.dependencies, 'S-008')
  assert.notEqual(updated.updatedAt, updated.createdAt)
})

test('updateStoryboardStory for a missing id returns not-found', async () => {
  const f = new FakeDb()
  await assert.rejects(
    updateStoryboardStory('NOPE', baseInput, f.tx),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'not-found',
  )
})

test('setStoryboardStatus changes only the status', async () => {
  const f = new FakeDb()
  await createStoryboardStory(baseInput, f.tx)
  const updated = await setStoryboardStatus('CRM-19', 'Blocked', f.tx)
  assert.equal(updated.status, 'Blocked')
  assert.equal(updated.title, 'WhatsApp live connector')
  assert.equal(f.rows[0].status, 'Blocked')
})

test('setStoryboardStatus for a missing id returns not-found', async () => {
  const f = new FakeDb()
  await assert.rejects(
    setStoryboardStatus('NOPE', 'Blocked', f.tx),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'not-found',
  )
})

test('listStoryboardStories returns the seeded rows in order', async () => {
  const f = new FakeDb()
  await createStoryboardStory(baseInput, f.tx)
  await createStoryboardStory(
    { ...baseInput, id: 'OPS-07', workstream: 'Portal / Operations' },
    f.tx,
  )
  const rows = await listStoryboardStories(f.tx)
  assert.ok(rows)
  assert.equal(rows.length, 2)
  assert.equal(rows[0].id, 'CRM-19')
  assert.equal(rows[1].id, 'OPS-07')
})
