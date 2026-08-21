import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  createStoryboardStory,
  finishStoryRun,
  listStoryboardRuns,
  listStoryRuns,
  startStoryRun,
} from '../../db/storyboard'
import { PortalWriteError } from '../../lib/portal-write-error'
import type { QueryExecutor } from '../../db/query-executor'

// Minimal in-memory fake covering storyboard_story + storyboard_story_run for
// the agent execution lifecycle (start → finish → history).

type Row = Record<string, any>

const baseInput = {
  id: 'CRM-19',
  workstream: 'CRM',
  title: 'WhatsApp live connector',
  priority: 'High',
  status: 'Planned',
  notes: 'Human product context — must never be overwritten.',
  batch: null,
  goal: null,
  scope: null,
  acceptanceCriteria: null,
  dependencies: null,
  completion: 10,
  rollup: true,
  plannedStartAt: null,
  actualStartAt: null,
  completedAt: null,
}

class FakeDb {
  stories: Row[] = []
  runs: Row[] = []
  runSeq = 0
  runReady = true
  now = '2026-08-21T12:00:00Z'

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
      if (t.includes('storyboard_story_run')) {
        return Promise.resolve([{ ready: this.runReady }])
      }
      return Promise.resolve([{ ready: true }])
    }

    if (t.includes('insert into storyboard_story_run')) {
      this.runSeq += 1
      const row = {
        id: `run-${this.runSeq}`,
        story_id: p[0],
        started_at: this.now,
        ended_at: null,
        result_status: null,
        completion: null,
        notes: null,
        commit_hash: null,
        tests_summary: null,
        created_at: this.now,
      }
      this.runs.push(row)
      return Promise.resolve([row])
    }

    if (t.includes('insert into storyboard_story ')) {
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
        completion: p[11],
        rollup: p[12],
        planned_start_at: p[13] ?? null,
        actual_start_at: p[14] ?? null,
        completed_at: p[15] ?? null,
        created_at: this.now,
        updated_at: this.now,
      }
      this.stories.push(row)
      return Promise.resolve([row])
    }

    if (t.includes('update storyboard_story_run')) {
      const r = this.runs.find((x) => x.id === p[5])
      if (!r) return Promise.resolve([])
      r.ended_at = this.now
      r.result_status = p[0]
      r.completion = p[1]
      r.notes = p[2]
      r.commit_hash = p[3] ?? null
      r.tests_summary = p[4] ?? null
      return Promise.resolve([r])
    }

    if (t.includes('update storyboard_story')) {
      if (t.includes("set status = 'in progress'")) {
        const r = this.stories.find((x) => x.id === p[0])
        if (!r) return Promise.resolve([])
        r.status = 'In Progress'
        r.actual_start_at = r.actual_start_at ?? this.now
        r.updated_at = this.now
        return Promise.resolve([r])
      }
      if (t.includes('set status = $1, completion = $2')) {
        const r = this.stories.find((x) => x.id === p[3])
        if (!r) return Promise.resolve([])
        r.status = p[0]
        r.completion = p[1]
        r.completed_at = p[0] === 'Complete' ? this.now : null
        r.updated_at = this.now
        return Promise.resolve([r])
      }
      throw new Error(`FAKE_UNHANDLED story update: ${t}`)
    }

    if (t.includes('from storyboard_story_run')) {
      if (t.includes('where story_id')) {
        return Promise.resolve(this.runs.filter((x) => x.story_id === p[0]))
      }
      return Promise.resolve(this.runs)
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }
}

test('startStoryRun sets In Progress, preserves first actual start, and creates a run', async () => {
  const f = new FakeDb()
  await createStoryboardStory(baseInput, f.tx)

  const first = await startStoryRun('CRM-19', f.tx)
  assert.equal(first.story.status, 'In Progress')
  assert.equal(first.story.actualStartAt, f.now)
  assert.equal(first.run.storyId, 'CRM-19')
  assert.equal(first.run.endedAt, null)
  assert.equal(first.run.resultStatus, null)
  assert.equal(first.story.notes, baseInput.notes, 'human notes untouched')

  // Retry: second run created, actual start preserved.
  const second = await startStoryRun('CRM-19', f.tx)
  assert.equal(second.story.actualStartAt, f.now)
  assert.equal(f.runs.length, 2)
  assert.notEqual(second.run.id, first.run.id)
})

test('finishStoryRun records history and updates the parent story', async () => {
  const f = new FakeDb()
  await createStoryboardStory(baseInput, f.tx)
  const { run } = await startStoryRun('CRM-19', f.tx)

  const { run: finished, story } = await finishStoryRun(
    run.id,
    {
      resultStatus: 'Partial',
      completion: 60,
      notes: 'Implemented webhook scaffolding; signature verification pending.',
      commitHash: 'abc123def456',
      testsSummary: '9/9 storyboard tests pass',
    },
    f.tx,
  )
  assert.equal(finished.endedAt, f.now)
  assert.equal(finished.resultStatus, 'Partial')
  assert.equal(finished.completion, 60)
  assert.equal(
    finished.notes,
    'Implemented webhook scaffolding; signature verification pending.',
  )
  assert.equal(finished.commitHash, 'abc123def456')
  assert.equal(finished.testsSummary, '9/9 storyboard tests pass')

  assert.equal(story.status, 'Partial')
  assert.equal(story.completion, 60)
  assert.equal(story.completedAt, null)
  assert.equal(story.notes, baseInput.notes, 'human notes never overwritten')
})

test('finishStoryRun with Complete forces completion 100 and sets completed_at', async () => {
  const f = new FakeDb()
  await createStoryboardStory(baseInput, f.tx)
  const { run } = await startStoryRun('CRM-19', f.tx)

  const { run: finished, story } = await finishStoryRun(
    run.id,
    {
      resultStatus: 'Complete',
      completion: 90,
      notes: 'Connector live on DEV.',
      commitHash: null,
      testsSummary: null,
    },
    f.tx,
  )
  assert.equal(finished.completion, 90, 'run keeps the agent-reported value')
  assert.equal(story.status, 'Complete')
  assert.equal(story.completion, 100, 'Complete forces 100')
  assert.equal(story.completedAt, f.now)
})

test('multiple runs for one story preserve history', async () => {
  const f = new FakeDb()
  await createStoryboardStory(baseInput, f.tx)

  const r1 = await startStoryRun('CRM-19', f.tx)
  await finishStoryRun(
    r1.run.id,
    {
      resultStatus: 'Blocked',
      completion: 30,
      notes: 'Provider unreachable.',
      commitHash: null,
      testsSummary: null,
    },
    f.tx,
  )
  const r2 = await startStoryRun('CRM-19', f.tx)
  await finishStoryRun(
    r2.run.id,
    {
      resultStatus: 'Partial',
      completion: 55,
      notes: 'Retry: adapter wired.',
      commitHash: null,
      testsSummary: null,
    },
    f.tx,
  )

  const storyRuns = await listStoryRuns('CRM-19', f.tx)
  assert.equal(storyRuns.length, 2)

  const all = await listStoryboardRuns(f.tx)
  assert.equal(all?.length, 2)
})

test('listStoryboardRuns returns null when the run table is missing', async () => {
  const f = new FakeDb()
  f.runReady = false
  assert.equal(await listStoryboardRuns(f.tx), null)
})

test('starting a run on a missing story returns not-found', async () => {
  const f = new FakeDb()
  await assert.rejects(
    startStoryRun('NOPE', f.tx),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'not-found',
  )
})
