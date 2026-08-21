import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  createStoryboardStory,
  finishStoryRun,
  listStoryboardRuns,
  listStoryRuns,
  startStoryRun,
  updateStoryboardStory,
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
  dependencies: null,
  preconditions: null,
  architectBrief: null,
  contextRefs: null,
  acceptanceCriteria: null,
  postconditions: null,
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
        goal_snapshot: p[1] ?? null,
        preconditions_snapshot: p[2] ?? null,
        architect_brief_snapshot: p[3] ?? null,
        context_refs_snapshot: p[4] ?? null,
        acceptance_criteria_snapshot: p[5] ?? null,
        postconditions_snapshot: p[6] ?? null,
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
        dependencies: p[9] ?? null,
        preconditions: p[10] ?? null,
        architect_brief: p[11] ?? null,
        context_refs: p[12] ?? null,
        acceptance_criteria: p[13] ?? null,
        postconditions: p[14] ?? null,
        architect_brief_updated_at: p[15] ? this.now : null,
        completion: p[16],
        rollup: p[17],
        planned_start_at: p[18] ?? null,
        actual_start_at: p[19] ?? null,
        completed_at: p[20] ?? null,
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
      if (t.includes('set workstream')) {
        const r = this.stories.find((x) => x.id === p[18])
        if (!r) return Promise.resolve([])
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
        r.completion = p[15]
        r.rollup = p[16]
        r.planned_start_at = p[17] ?? null
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

test('starting a run snapshots the execution specification', async () => {
  const f = new FakeDb()
  const spec = {
    goal: 'Deliver WhatsApp receipts reliably.',
    preconditions: 'Provider webhook approved.',
    architectBrief: 'Reuse the media provider seam; keep workflow_engine generic.',
    contextRefs: 'workflow_app/…, db/migrations/021_*.sql',
    acceptanceCriteria: 'Idempotent receipt recorded for every webhook hit.',
    postconditions: 'Human story notes preserved; rollup math unchanged.',
  }
  await createStoryboardStory({ ...baseInput, ...spec }, f.tx)

  const { run } = await startStoryRun('CRM-19', f.tx)
  assert.equal(run.goalSnapshot, spec.goal)
  assert.equal(run.preconditionsSnapshot, spec.preconditions)
  assert.equal(run.architectBriefSnapshot, spec.architectBrief)
  assert.equal(run.contextRefsSnapshot, spec.contextRefs)
  assert.equal(run.acceptanceCriteriaSnapshot, spec.acceptanceCriteria)
  assert.equal(run.postconditionsSnapshot, spec.postconditions)
})

test('historical snapshots are immutable; a later run captures the newer spec', async () => {
  const f = new FakeDb()
  const specV1 = {
    goal: 'Goal v1',
    preconditions: 'Preconditions v1',
    architectBrief: 'Brief v1',
    contextRefs: 'Refs v1',
    acceptanceCriteria: 'Criteria v1',
    postconditions: 'Postconditions v1',
  }
  await createStoryboardStory({ ...baseInput, ...specV1 }, f.tx)

  const first = await startStoryRun('CRM-19', f.tx)

  // The human/architect edits the parent story after run 1 has started.
  const specV2 = {
    goal: 'Goal v2',
    preconditions: 'Preconditions v2',
    architectBrief: 'Brief v2',
    contextRefs: 'Refs v2',
    acceptanceCriteria: 'Criteria v2',
    postconditions: 'Postconditions v2',
  }
  await updateStoryboardStory('CRM-19', { ...baseInput, ...specV2 }, f.tx)

  // Editing the parent must NOT alter the historical snapshot.
  const afterEdit = await listStoryRuns('CRM-19', f.tx)
  const run1 = afterEdit[0]
  assert.equal(run1.goalSnapshot, specV1.goal)
  assert.equal(run1.preconditionsSnapshot, specV1.preconditions)
  assert.equal(run1.architectBriefSnapshot, specV1.architectBrief)
  assert.equal(run1.contextRefsSnapshot, specV1.contextRefs)
  assert.equal(run1.acceptanceCriteriaSnapshot, specV1.acceptanceCriteria)
  assert.equal(run1.postconditionsSnapshot, specV1.postconditions)

  // A second run receives the newer/current specification.
  const second = await startStoryRun('CRM-19', f.tx)
  assert.equal(second.run.goalSnapshot, specV2.goal)
  assert.equal(second.run.architectBriefSnapshot, specV2.architectBrief)
  assert.equal(second.run.postconditionsSnapshot, specV2.postconditions)

  // Run 1 still retains the ORIGINAL specification.
  const finalRuns = await listStoryRuns('CRM-19', f.tx)
  assert.equal(finalRuns.length, 2)
  const firstRun = finalRuns.find((r) => r.goalSnapshot === specV1.goal)
  assert.ok(firstRun)
  assert.equal(firstRun.architectBriefSnapshot, specV1.architectBrief)
  assert.equal(firstRun.postconditionsSnapshot, specV1.postconditions)
  const secondRun = finalRuns.find((r) => r.goalSnapshot === specV2.goal)
  assert.ok(secondRun)
  assert.equal(secondRun.architectBriefSnapshot, specV2.architectBrief)
})
