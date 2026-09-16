import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import {
  createStoryboardStory,
  getStoryboardStory,
  isStoryboardTableReady,
  listStoryboardStories,
  setStoryboardStatus,
  storyCompletionForRun,
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

    if (t.includes('information_schema.columns')) {
      // ENG-FORGE-V6-VIS probe: this story-level fake has no run table, so
      // report the V6 columns absent and stay on the legacy path.
      return Promise.resolve([{ count: 0 }])
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
        test_mode: p[16] ?? null,
        assay_commands: p[17] ?? null,
        packet_sha: p[18] ?? null,
        completion: p[19],
        rollup: p[20],
        planned_start_at: p[21] ?? null,
        actual_start_at: p[22] ?? null,
        completed_at: p[23] ?? null,
        operating_surface: p[24] ?? null,
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
      // so the story id is $3 (p[2]); full update has 23 params with the id
      // last as $23 (p[22]) and no longer touches the system-owned
      // actual_start_at / completed_at.
      const id = statusOnly ? p[2] : p[22]
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
        // from $18 then now() else architect_brief_updated_at end — the CASE
        // re-binds the brief as p[17] (after test_mode p[14], assay p[15],
        // packet p[16]); compare it to the PRIOR stored value.
        if ((p[17] ?? null) !== priorBrief) {
          r.architect_brief_updated_at = this.stamp()
        }
        r.test_mode = p[14] ?? r.test_mode ?? null
        r.assay_commands = p[15] ?? r.assay_commands ?? null
        r.packet_sha = p[16] ?? r.packet_sha ?? null
        r.completion = p[18]
        r.rollup = p[19]
        r.planned_start_at = p[20] ?? null
        // operating_surface is $22 (p[21]); where id is $23 (p[22]).
        r.operating_surface = p[21] ?? null
      }
      r.updated_at = '2026-08-21T01:00:00Z'
      return Promise.resolve([r])
    }

    if (t.includes('from storyboard_story')) {
      if (t.includes('where id =')) {
        const row = this.rows.find((r) => r.id === p[0])
        return Promise.resolve(row ? [row] : [])
      }
      return Promise.resolve(this.rows)
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }
}

const baseInput = {
  id: 'CRM-19',
  workstream: 'CRM',
  operatingSurface: null,
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

test('getStoryboardStory returns a story by id and null for unknown ids', async () => {
  const f = new FakeDb()
  await createStoryboardStory({ ...baseInput, id: 'ENG-04', status: 'Planned' }, f.tx)
  await createStoryboardStory({ ...baseInput, id: 'CRM-14B', status: 'Blocked' }, f.tx)

  const found = await getStoryboardStory('ENG-04', f.tx)
  assert.equal(found?.id, 'ENG-04')
  assert.equal(found?.status, 'Planned')
  const foundTwo = await getStoryboardStory('CRM-14B', f.tx)
  assert.equal(foundTwo?.id, 'CRM-14B')
  const missing = await getStoryboardStory('NOPE', f.tx)
  assert.equal(missing, null)
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

// ---------------------------------------------------------------------------
// ASTRA item 6 / migration 182 — 100 IS ONLY TRUE WITH `Complete`, IN THE SAME WRITE.
//
// Measured live on 2026-09-16: the QA lane of ENG-QA-SINGLE-VERDICT-01 finished with the verdict
// `Hold` / CANDIDATE_MISMATCH and completion 100 OF ITS OWN RUN, and `finishStoryRun` copied that 100
// onto the story. The database refused the pair
// (`storyboard_story_completion_requires_complete`), `work.finish` threw a raw constraint error, and the
// lane died without recording its verdict (engine task 4f1a0e66). The rule was already exported
// (`completionIsLegal`) and the writer was not calling it. These tests pin the pair.
// ---------------------------------------------------------------------------

test('storyCompletionForRun: a Complete run is the only writer of 100', () => {
  assert.deepEqual(storyCompletionForRun('Complete', 100), {
    status: 'Complete',
    completion: 100,
  })
})

test('storyCompletionForRun: a Hold run cannot claim 100 for the story', () => {
  // The exact live case: the Assay ran every frozen command (its OWN completion is 100) and the verdict
  // was Hold. The story is not finished, so it must not read 100.
  const pair = storyCompletionForRun('Hold', 100)
  assert.equal(pair.status, 'Hold')
  assert.equal(pair.completion, 0, 'the story claims nothing rather than a number nobody measured')
})

test('storyCompletionForRun: a Cancelled run becomes a Hold story, still never 100', () => {
  const pair = storyCompletionForRun('Cancelled', 100)
  assert.equal(pair.status, 'Hold')
  assert.equal(pair.completion, 0)
})

test('storyCompletionForRun: a run below 100 is carried through unchanged', () => {
  assert.deepEqual(storyCompletionForRun('In Progress', 40), {
    status: 'In Progress',
    completion: 40,
  })
  assert.deepEqual(storyCompletionForRun('Failed', 0), {
    status: 'Failed',
    completion: 0,
  })
})

test('storyCompletionForRun: a run that reported no number gets 0, not null', () => {
  // `storyboard_story.completion` is NOT NULL with a default of 0 — writing null would be a second crash.
  for (const missing of [null, undefined]) {

// ---------------------------------------------------------------------------
// THE ARCHITECT'S OWN WRITER MAINTAINS ITS OWN TIMESTAMP.
//
// Migration 024 added `architect_brief_updated_at` for one fact: WHEN the brief last changed. The board
// editor (`updateStoryboardStory`) stamps it. The writer the ARCHITECT lane actually goes through
// (`setStoryArchitectBrief`) did not, so every engine-written brief carried the contract and a null
// timestamp — measured 2026-09-16, six of six completed stories — and "when did this contract land" was
// unanswerable. A column that only one of its two writers maintains is a column that lies.
//
// A source fence rather than a fake-level test: the FakeDb models the board editor's CASE, and teaching it a
// second statement would model the SQL under test, which is how the original gap stayed invisible.
// ---------------------------------------------------------------------------

test('the architect brief writer stamps architect_brief_updated_at when the brief changes', () => {
  const source = readFileSync(new URL('../../db/storyboard.ts', import.meta.url), 'utf8')
  const at = source.indexOf('export async function setStoryArchitectBrief')
  assert.ok(at > 0, 'setStoryArchitectBrief must exist')
  const body = source.slice(at, source.indexOf('return rows.length > 0', at))

  assert.match(body, /architect_brief_updated_at = case/, 'the brief writer must stamp the change time')
  assert.match(
    body,
    /when architect_brief is distinct from/,
    'and it must stamp ONLY when the brief actually changed — the same rule the board editor uses',
  )
})

    assert.deepEqual(storyCompletionForRun('Hold', missing), {
      status: 'Hold',
      completion: 0,
    })
  }
})

test('storyCompletionForRun: every pair it returns is legal by the exported rule', () => {
  for (const status of ['Complete', 'Hold', 'In Progress', 'Failed', 'Cancelled']) {
    for (const runCompletion of [null, 0, 40, 99, 100]) {
      const pair = storyCompletionForRun(status, runCompletion)
      assert.ok(
        pair.completion === 100 ? pair.status === 'Complete' : true,
        `${status}/${runCompletion} produced ${pair.completion} with ${pair.status}`,
      )
    }
  }
})

