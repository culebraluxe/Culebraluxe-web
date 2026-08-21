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
  stampSeq = 0

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  private stamp(): string {
    this.stampSeq += 1
    return `2026-08-21T02:00:0${this.stampSeq}Z`
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
        dependencies: p[9] ?? null,
        preconditions: p[10] ?? null,
        architect_brief: p[11] ?? null,
        context_refs: p[12] ?? null,
        acceptance_criteria: p[13] ?? null,
        postconditions: p[14] ?? null,
        architect_brief_updated_at: p[15] ? this.stamp() : null,
        completion: p[16],
        rollup: p[17],
        planned_start_at: p[18] ?? null,
        actual_start_at: p[19] ?? null,
        completed_at: p[20] ?? null,
        created_at: '2026-08-21T00:00:00Z',
        updated_at: '2026-08-21T00:00:00Z',
      }
      this.rows.push(row)
      return Promise.resolve([row])
    }

    if (t.includes('update storyboard_story')) {
      const statusOnly =
        t.includes('set status = $1') && !t.includes('set workstream')
      // status-only update: `set status=$1, completion=case when $2='Complete'...`
      // so the story id is $3 (p[2]); full update has id as $19 (p[18]) and no
      // longer touches the system-owned actual_start_at / completed_at.
      const id = statusOnly ? p[2] : p[18]
      const r = this.rows.find((x) => x.id === id)
      if (!r) return Promise.resolve([])
      if (statusOnly) {
        r.status = p[0]
        // Complete forces completion = 100 (mirrors the repository SQL).
        if (p[0] === 'Complete') r.completion = 100
      } else {
        const priorBrief = r.architect_brief ?? null
        r.workstream = p[0]
        r.title = p[1]
        r.priority = p[2]
        r.status = p[3]
        r.notes = p[4]
        r.batch = p[5] ?? null
        r.goal = p[6] ?? null
        r.scope = p[7] ?? null
        r.dependencies = p[8] ?? null
        r.preconditions = p[9] ?? null
        r.architect_brief = p[10] ?? null
        r.context_refs = p[11] ?? null
        r.acceptance_criteria = p[12] ?? null
        r.postconditions = p[13] ?? null
        // architect_brief_updated_at = case when architect_brief is distinct
        // from $14 then now() else architect_brief_updated_at end — compares
        // the incoming brief (p[14] === p[10]) to the PRIOR stored value.
        if ((p[14] ?? null) !== priorBrief) {
          r.architect_brief_updated_at = this.stamp()
        }
        r.completion = p[15]
        r.rollup = p[16]
        r.planned_start_at = p[17] ?? null
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
  workstream: 'CRM',
  title: 'WhatsApp live connector',
  priority: 'High',
  status: 'Open',
  notes: 'Provider webhook + signature verification.',
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
  rollup: true,
  plannedStartAt: null,
  actualStartAt: null,
  completedAt: null,
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
  assert.equal(story.workstream, 'CRM')
  assert.equal(story.title, 'WhatsApp live connector')
  assert.equal(story.priority, 'High')
  assert.equal(story.status, 'Open')
  assert.equal(story.notes, 'Provider webhook + signature verification.')
  assert.equal(story.acceptanceCriteria, null)
  assert.equal(story.completion, 0)
  assert.equal(story.rollup, true)
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
      workstream: 'TXN',
      title: 'WhatsApp connector (renamed)',
      priority: 'Critical',
      status: 'Blocked',
      notes: 'Needs the S-008 channel decision.',
      batch: 4,
      goal: 'Lower provider deliveries.',
      scope: 'Webhook only.',
      acceptanceCriteria: 'Idempotent receipt.',
      dependencies: 'S-008',
      completion: 85,
      rollup: true,
      plannedStartAt: '2026-08-22',
      actualStartAt: '2026-08-23',
      completedAt: null,
    },
    f.tx,
  )
  assert.equal(updated.id, 'CRM-19')
  assert.equal(updated.workstream, 'TXN')
  assert.equal(updated.title, 'WhatsApp connector (renamed)')
  assert.equal(updated.priority, 'Critical')
  assert.equal(updated.status, 'Blocked')
  assert.equal(updated.batch, 4)
  assert.equal(updated.goal, 'Lower provider deliveries.')
  assert.equal(updated.dependencies, 'S-008')
  assert.equal(updated.completion, 85)
  assert.equal(updated.rollup, true)
  assert.equal(updated.plannedStartAt, '2026-08-22')
  // actual_start_at and completed_at are system-owned dates: the normal edit
  // path never touches them, even when the form payload carries stale values.
  assert.equal(updated.actualStartAt, null)
  assert.equal(updated.completedAt, null)
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

test('execution specification fields persist through create and update', async () => {
  const f = new FakeDb()
  const spec = {
    goal: 'Deliver WhatsApp receipts reliably.',
    dependencies: 'CRM-08',
    preconditions: 'Provider webhook approved.',
    architectBrief: 'Reuse the media provider seam; do not touch workflow_engine.',
    contextRefs: 'workflow_app/…, db/migrations/021_*.sql',
    acceptanceCriteria: 'Idempotent receipt recorded for every webhook hit.',
    postconditions: 'storyboard_story rows untouched; human notes preserved.',
  }
  const created = await createStoryboardStory(
    { ...baseInput, ...spec },
    f.tx,
  )
  assert.equal(created.goal, spec.goal)
  assert.equal(created.dependencies, spec.dependencies)
  assert.equal(created.preconditions, spec.preconditions)
  assert.equal(created.architectBrief, spec.architectBrief)
  assert.equal(created.contextRefs, spec.contextRefs)
  assert.equal(created.acceptanceCriteria, spec.acceptanceCriteria)
  assert.equal(created.postconditions, spec.postconditions)
  assert.notEqual(created.architectBriefUpdatedAt, null)

  const updated = await updateStoryboardStory(
    'CRM-19',
    {
      ...baseInput,
      ...spec,
      architectBrief: 'New architect direction after review.',
      postconditions: 'Updated invariant.',
    },
    f.tx,
  )
  assert.equal(updated.architectBrief, 'New architect direction after review.')
  assert.equal(updated.postconditions, 'Updated invariant.')
  assert.equal(updated.goal, spec.goal)
})

test('architect_brief_updated_at stamps only when the architect brief changes', async () => {
  const f = new FakeDb()
  await createStoryboardStory(baseInput, f.tx)
  assert.equal(f.rows[0].architect_brief_updated_at, null)

  // Changing unrelated fields must NOT stamp architect_brief_updated_at.
  const unrelated = await updateStoryboardStory(
    'CRM-19',
    { ...baseInput, title: 'Renamed title', notes: 'Human note update.' },
    f.tx,
  )
  assert.equal(unrelated.architectBriefUpdatedAt, null)

  // Changing the architect brief stamps it.
  const briefed = await updateStoryboardStory(
    'CRM-19',
    { ...baseInput, architectBrief: 'First brief.' },
    f.tx,
  )
  assert.notEqual(briefed.architectBriefUpdatedAt, null)
  const stampedAt = briefed.architectBriefUpdatedAt

  // A later unrelated edit must not move the stamp.
  const after = await updateStoryboardStory(
    'CRM-19',
    { ...baseInput, architectBrief: 'First brief.', priority: 'Low' },
    f.tx,
  )
  assert.equal(after.architectBriefUpdatedAt, stampedAt)

  // Changing the brief again re-stamps.
  const rebriefed = await updateStoryboardStory(
    'CRM-19',
    { ...baseInput, architectBrief: 'Second brief.' },
    f.tx,
  )
  assert.notEqual(rebriefed.architectBriefUpdatedAt, stampedAt)
})
