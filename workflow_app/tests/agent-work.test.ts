import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  beginAgentWorkRun,
  cancelAgentWork,
  claimNextAgentWork,
  failAgentWork,
  finishAgentWork,
  getActiveAgentWorkItem,
  getAgentWorkItem,
  listActiveAgentWorkForStory,
  listStaleAgentWork,
  recoverStaleAgentWork,
  updateAgentWorkProgress,
} from '../../db/agent-work'
import {
  createStoryboardStory,
  setStoryboardStatus,
  type StoryboardStoryInput,
} from '../../db/storyboard'
import { PortalWriteError } from '../../lib/portal-write-error'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'

// ---------------------------------------------------------------------------
// Work-queue tests (migration 025). In-memory fake covering storyboard_story,
// storyboard_story_run, and agent_work_item — including the dispatch trigger
// behavior (status INTO Ready creates one Ready work item) and the claim
// semantics (single worker, priority DESC then queued_at ASC). No database.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

const baseInput: StoryboardStoryInput = {
  id: 'ENG-04',
  workstream: 'HARDEN',
  operatingSurface: 'TECH',
  title: 'TUNIT Formal Regression Suite',
  priority: 'High',
  status: 'Planned',
  notes: 'Human product notes — preserved.',
  batch: null,
  goal: 'Durable regression suite behind one canonical command.',
  scope: null,
  dependencies: null,
  preconditions: null,
  architectBrief: 'Keep workflow_engine generic; reuse in-memory fakes.',
  contextRefs: null,
  acceptanceCriteria: 'One canonical command runs the whole suite.',
  postconditions: null,
  completion: 15,
  rollup: true,
  plannedStartAt: null,
  actualStartAt: null,
  completedAt: null,
}

class FakeQueueDb {
  stories: Row[] = []
  runs: Row[] = []
  workItems: Row[] = []
  runSeq = 0
  workSeq = 0
  now = '2026-08-21T12:00:00Z'

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  /** Emulates migration 025's dispatch trigger: status INTO Ready → one item. */
  private dispatchIfReady(story: Row): void {
    if (story.status === 'Ready') {
      const existingActive = this.workItems.some(
        (w) =>
          w.story_id === story.id &&
          ['Ready', 'Claimed', 'Running'].includes(w.state),
      )
      if (!existingActive) {
        this.workSeq += 1
        this.workItems.push({
          id: `work-${this.workSeq}`,
          story_id: story.id,
          state: 'Ready',
          priority: story.priority_score ?? 0,
          queued_at: this.now,
          claimed_at: null,
          claimed_by: null,
          started_at: null,
          finished_at: null,
          story_run_id: null,
          error_text: null,
          created_at: this.now,
          updated_at: this.now,
        })
      }
    }
  }

  /** Direct enqueue with an explicit priority (emulates the trigger priority). */
  enqueue(storyId: string, priority = 0): void {
    this.workSeq += 1
    this.workItems.push({
      id: `work-${this.workSeq}`,
      story_id: storyId,
      state: 'Ready',
      priority,
      queued_at: this.now,
      claimed_at: null,
      claimed_by: null,
      started_at: null,
      finished_at: null,
      story_run_id: null,
      error_text: null,
      created_at: this.now,
      updated_at: this.now,
    })
  }

  runner: TxRunner = async (cb) => cb(this.tx)

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''),
        '',
      ),
    )
    const p = params as any[]

    if (t.includes('to_regclass')) {
      return Promise.resolve([{ ready: true }])
    }
    if (t.includes('pg_advisory_xact_lock')) {
      return Promise.resolve([])
    }

    // ---- storyboard_story ----
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
        operating_surface: p[21] ?? null,
        created_at: this.now,
        updated_at: this.now,
        priority_score: 0,
      }
      this.stories.push(row)
      this.dispatchIfReady(row)
      return Promise.resolve([row])
    }

    // ---- storyboard_story_run ---- (before story UPDATE: the story UPDATE
    // check would shadow `update storyboard_story_run` as a substring).
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
        updated_at: this.now,
      }
      this.runs.push(row)
      return Promise.resolve([row])
    }
    if (t.includes('update storyboard_story_run')) {
      // Each CASE expression repeats its parameter, so param positions are
      // fixed by the SQL in db/storyboard.ts:
      //   finish:      [status, completion, notes×4, commit, tests, id]
      //   progress:    [completion×2, note×4, tests×2, id]
      //   terminate:   [status, completion×2, note×2, tests×2, id]
      const runId = p[p.length - 1]
      const r = this.runs.find((x) => x.id === runId)
      if (!r) return Promise.resolve([])
      const append = (note: string) => {
        r.notes = r.notes ? `${r.notes}\n${note}` : note
      }
      if (t.includes('ended_at = now()') && t.includes('completion = case when')) {
        // terminateStoryRun (fail/cancel/stale)
        r.ended_at = this.now
        r.result_status = p[0]
        if (p[1] !== null) r.completion = p[1]
        if (p[3]) append(p[3])
        if (p[5] !== null) r.tests_summary = p[5]
      } else if (!t.includes('ended_at = now()')) {
        // updateStoryRunProgress
        if (p[0] !== null) r.completion = p[0]
        if (p[2]) append(p[2])
        if (p[6] !== null) r.tests_summary = p[6]
      } else {
        // finishStoryRun
        r.ended_at = this.now
        r.result_status = p[0]
        r.completion = p[1]
        if (p[2]) append(p[2])
        if (p[6] !== null) r.commit_hash = p[6]
        if (p[7] !== null) r.tests_summary = p[7]
      }
      r.updated_at = this.now
      return Promise.resolve([r])
    }
    if (t.includes('from storyboard_story_run')) {
      if (t.includes('where story_id')) {
        return Promise.resolve(this.runs.filter((x) => x.story_id === p[0]))
      }
      return Promise.resolve(this.runs)
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
      // setStoryboardStatus (status-only update): id is p[2].
      if (t.includes('set status = $1') && t.includes('completion = case')) {
        const r = this.stories.find((x) => x.id === p[2])
        if (!r) return Promise.resolve([])
        r.status = p[0]
        if (p[0] === 'Complete') r.completion = 100
        r.updated_at = this.now
        this.dispatchIfReady(r)
        return Promise.resolve([r])
      }
      throw new Error(`FAKE_UNHANDLED story update: ${t}`)
    }

    if (t.includes('from storyboard_story')) {
      if (t.includes('where id =')) {
        const row = this.stories.find((x) => x.id === p[0])
        return Promise.resolve(row ? [row] : [])
      }
      return Promise.resolve(this.stories)
    }

    // ---- agent_work_item ----
    // Updates first: the claim UPDATE contains `from agent_work_item` in its
    // subquery, so update handlers must be checked before the select branch.
    // Heartbeat (updateAgentWorkProgress): only updated_at changes.
    if (
      t.includes('update agent_work_item') &&
      t.includes('set updated_at = now()')
    ) {
      const r = this.workItems.find((x) => x.id === p[0])
      if (!r || r.state !== 'Running') return Promise.resolve([])
      r.updated_at = this.now
      return Promise.resolve([r])
    }
    if (t.includes('update agent_work_item') && t.includes("state = 'claimed'")) {
      const active = this.workItems.some((w) =>
        ['Claimed', 'Running'].includes(w.state),
      )
      if (active) return Promise.resolve([])
      const ready = this.workItems
        .filter((w) => w.state === 'Ready')
        .sort((a, b) =>
          b.priority - a.priority || a.queued_at.localeCompare(b.queued_at),
        )
      const target = ready[0]
      if (!target) return Promise.resolve([])
      target.state = 'Claimed'
      target.claimed_at = this.now
      target.claimed_by = p[0]
      target.updated_at = this.now
      return Promise.resolve([target])
    }

    // Begin execution: work item -> Running with story_run_id (p[0]); the
    // work item id is the last param (where id = $2).
    if (t.includes('update agent_work_item') && t.includes("state = 'running'")) {
      const r = this.workItems.find((x) => x.id === p[1])
      if (!r) return Promise.resolve([])
      r.state = 'Running'
      r.started_at = this.now
      r.story_run_id = p[0]
      r.updated_at = this.now
      return Promise.resolve([r])
    }

    // Finish: work item -> Done.
    if (t.includes('update agent_work_item') && t.includes("state = 'done'")) {
      const r = this.workItems.find((x) => x.id === p[0])
      if (!r) return Promise.resolve([])
      r.state = 'Done'
      r.finished_at = this.now
      r.updated_at = this.now
      return Promise.resolve([r])
    }

    // Fail: work item -> Error with error_text (p[0]); id is the last param.
    if (t.includes('update agent_work_item') && t.includes("state = 'error'")) {
      const r = this.workItems.find((x) => x.id === p[1])
      if (!r) return Promise.resolve([])
      r.state = 'Error'
      r.error_text = p[0]
      r.finished_at = this.now
      r.updated_at = this.now
      return Promise.resolve([r])
    }

    // Cancel: work item -> Cancelled.
    if (
      t.includes('update agent_work_item') &&
      t.includes("state = 'cancelled'")
    ) {
      const r = this.workItems.find((x) => x.id === p[0])
      if (!r) return Promise.resolve([])
      r.state = 'Cancelled'
      r.finished_at = this.now
      r.updated_at = this.now
      return Promise.resolve([r])
    }

    if (t.includes('from agent_work_item')) {
      if (t.includes('where story_id')) {
        return Promise.resolve(
          this.workItems.filter((x) => x.story_id === p[0]),
        )
      }
      if (t.includes('updated_at <')) {
        // Stale scan (listStaleAgentWork / recoverStaleAgentWork): Claimed or
        // Running with a heartbeat older than the cutoff (minutes param p[0]).
        const cutoff = Date.parse(this.now) - (p[0] as number) * 60_000
        return Promise.resolve(
          this.workItems
            .filter(
              (x) =>
                ['Claimed', 'Running'].includes(x.state) &&
                Date.parse(x.updated_at) < cutoff,
            )
            .sort((a, b) => a.updated_at.localeCompare(b.updated_at)),
        )
      }
      if (t.includes('where state in')) {
        return Promise.resolve(
          this.workItems.filter((x) =>
            ['Claimed', 'Running'].includes(x.state),
          ),
        )
      }
      if (t.includes('where id =')) {
        const row = this.workItems.find((x) => x.id === p[0])
        return Promise.resolve(row ? [row] : [])
      }
      return Promise.resolve(this.workItems)
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }
}

async function seedStory(
  f: FakeQueueDb,
  overrides: Partial<StoryboardStoryInput> = {},
): Promise<string> {
  const id = overrides.id ?? 'ENG-04'
  await createStoryboardStory({ ...baseInput, ...overrides, id }, f.tx)
  return id
}

test('Planned -> Ready dispatches exactly one Ready work item', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)
  await setStoryboardStatus(id, 'Ready', f.tx)

  const items = f.workItems.filter((w) => w.story_id === id)
  assert.equal(items.length, 1)
  assert.equal(items[0].state, 'Ready')
  assert.equal(items[0].queued_at, f.now)
})

test('editing a story while it stays Ready creates no duplicate work item', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)
  await setStoryboardStatus(id, 'Ready', f.tx)
  await setStoryboardStatus(id, 'Ready', f.tx)

  const items = f.workItems.filter((w) => w.story_id === id)
  assert.equal(items.length, 1)
})

test('Ready -> other state -> Ready again creates a NEW item only after history', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)
  await setStoryboardStatus(id, 'Ready', f.tx)

  const claim1 = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim1)
  const begun1 = await beginAgentWorkRun(claim1.workItem.id, f.tx)
  await finishAgentWork(
    begun1.workItem.id,
    {
      resultStatus: 'Partial',
      completion: 50,
      notes: 'Partial attempt.',
      commitHash: 'abc123',
      testsSummary: 'tests 10/10',
    },
    f.tx,
  )

  await setStoryboardStatus(id, 'Blocked', f.tx)
  await setStoryboardStatus(id, 'Ready', f.tx)

  const items = f.workItems.filter((w) => w.story_id === id)
  assert.equal(items.length, 2)
  assert.ok(items.some((w) => w.state === 'Done'))
  assert.ok(items.some((w) => w.state === 'Ready'))
  assert.equal(items[0].state, 'Done')
  assert.equal(items[1].state, 'Ready')
})

test('claim picks the highest priority Ready item first', async () => {
  const f = new FakeQueueDb()
  await seedStory(f, { id: 'A-1' })
  await seedStory(f, { id: 'A-2' })
  f.enqueue('A-1', 10)
  f.enqueue('A-2', 80)

  const claim = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim)
  assert.equal(claim.workItem.storyId, 'A-2', 'higher priority wins')
})

test('equal priority uses the oldest queued_at', async () => {
  const f = new FakeQueueDb()
  await seedStory(f, { id: 'B-1' })
  await seedStory(f, { id: 'B-2' })
  f.enqueue('B-1', 50)
  f.enqueue('B-2', 50)

  const claim = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim)
  assert.equal(claim.workItem.storyId, 'B-1', 'oldest queued_at wins')
})

test('second claim while an item is Claimed/Running returns no work', async () => {
  const f = new FakeQueueDb()
  await seedStory(f, { id: 'C-1' })
  await seedStory(f, { id: 'C-2' })
  f.enqueue('C-1')
  f.enqueue('C-2')

  const first = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(first)
  assert.equal(first.workItem.storyId, 'C-1')

  const second = await claimNextAgentWork('worker-2', f.runner)
  assert.equal(second, null, 'single-worker rule refuses the second claim')
})

test('begin creates a run with spec snapshot, story -> In Progress, work -> Running', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f, {
    goal: 'The goal.',
    preconditions: 'Pre.',
    architectBrief: 'Brief.',
    contextRefs: 'Refs.',
    acceptanceCriteria: 'Criteria.',
    postconditions: 'Post.',
  })
  await setStoryboardStatus(id, 'Ready', f.tx)
  const claim = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim)

  const begun = await beginAgentWorkRun(claim.workItem.id, f.tx)
  assert.equal(begun.workItem.state, 'Running')
  assert.ok(begun.workItem.storyRunId)
  assert.equal(begun.story.status, 'In Progress')
  assert.equal(begun.story.actualStartAt, f.now, 'first actual start preserved')

  const run = f.runs.find((r) => r.id === begun.workItem.storyRunId)
  assert.ok(run)
  assert.equal(run.goal_snapshot, 'The goal.')
  assert.equal(run.preconditions_snapshot, 'Pre.')
  assert.equal(run.architect_brief_snapshot, 'Brief.')
  assert.equal(run.context_refs_snapshot, 'Refs.')
  assert.equal(run.acceptance_criteria_snapshot, 'Criteria.')
  assert.equal(run.postconditions_snapshot, 'Post.')
})

test('finish persists run result, updates story, and marks work Done', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)
  await setStoryboardStatus(id, 'Ready', f.tx)
  const claim = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim)
  const begun = await beginAgentWorkRun(claim.workItem.id, f.tx)

  const finished = await finishAgentWork(
    begun.workItem.id,
    {
      resultStatus: 'Partial',
      completion: 60,
      notes: 'Scaffold done; acceptance pending.',
      commitHash: 'def456',
      testsSummary: '12/12 pass',
    },
    f.tx,
  )
  assert.equal(finished.workItem.state, 'Done')
  assert.ok(finished.workItem.finishedAt)
  assert.equal(finished.story.status, 'Partial')
  assert.equal(finished.story.completion, 60)

  const run = f.runs.find((r) => r.id === begun.workItem.storyRunId)
  assert.equal(run.result_status, 'Partial')
  assert.equal(run.notes, 'Scaffold done; acceptance pending.')
})

test('Complete result forces completion 100 and sets completed_at', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)
  await setStoryboardStatus(id, 'Ready', f.tx)
  const claim = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim)
  const begun = await beginAgentWorkRun(claim.workItem.id, f.tx)

  const finished = await finishAgentWork(
    begun.workItem.id,
    {
      resultStatus: 'Complete',
      completion: 100,
      notes: 'Done.',
      commitHash: null,
      testsSummary: null,
    },
    f.tx,
  )
  assert.equal(finished.story.status, 'Complete')
  assert.equal(finished.story.completion, 100)
  assert.equal(finished.story.completedAt, f.now)
})

test('failAgentWork without a begun run marks Error and leaves no run', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)
  await setStoryboardStatus(id, 'Ready', f.tx)
  const claim = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim)

  const failed = await failAgentWork(
    claim.workItem.id,
    'infrastructure failure: could not begin run',
    undefined,
    f.tx,
  )
  assert.equal(failed.state, 'Error')
  assert.ok(failed.errorText?.includes('infrastructure failure'))
  assert.ok(failed.finishedAt)
  assert.equal(f.runs.length, 0, 'no run existed; none created')
})

test('cancelAgentWork without a begun run marks the item Cancelled', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)
  await setStoryboardStatus(id, 'Ready', f.tx)
  const claim = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim)

  const cancelled = await cancelAgentWork(claim.workItem.id, undefined, f.tx)
  assert.equal(cancelled.state, 'Cancelled')
})

test('begin on a non-Claimed item is refused', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)
  await setStoryboardStatus(id, 'Ready', f.tx)
  const item = f.workItems.find((w) => w.story_id === id)!
  await assert.rejects(
    beginAgentWorkRun(item.id, f.tx),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'conflict',
  )
})

test('finish without a story run is refused', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)
  await setStoryboardStatus(id, 'Ready', f.tx)
  const item = f.workItems.find((w) => w.story_id === id)!
  await assert.rejects(
    finishAgentWork(
      item.id,
      {
        resultStatus: 'Partial',
        completion: 50,
        notes: '',
        commitHash: null,
        testsSummary: null,
      },
      f.tx,
    ),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'conflict',
  )
})

test('listActiveAgentWorkForStory returns only active items', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)
  await setStoryboardStatus(id, 'Ready', f.tx)

  const active = await listActiveAgentWorkForStory(id, f.tx)
  assert.equal(active.length, 1)
  assert.equal(active[0].state, 'Ready')

  const claim = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim)
  const activeAfterClaim = await listActiveAgentWorkForStory(id, f.tx)
  assert.equal(activeAfterClaim.length, 1)
  assert.equal(activeAfterClaim[0].state, 'Claimed')
  assert.equal(activeAfterClaim[0].claimedBy, 'worker-1')
})

test('work item and run history survive retries of a Partial story', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f)

  await setStoryboardStatus(id, 'Ready', f.tx)
  const claim1 = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim1)
  const begun1 = await beginAgentWorkRun(claim1.workItem.id, f.tx)
  await finishAgentWork(
    begun1.workItem.id,
    {
      resultStatus: 'Partial',
      completion: 50,
      notes: 'First attempt partial.',
      commitHash: 'a1',
      testsSummary: 't1',
    },
    f.tx,
  )

  await setStoryboardStatus(id, 'Blocked', f.tx)
  await setStoryboardStatus(id, 'Ready', f.tx)
  const claim2 = await claimNextAgentWork('worker-2', f.runner)
  assert.ok(claim2)
  assert.notEqual(claim2.workItem.id, claim1.workItem.id)
  const begun2 = await beginAgentWorkRun(claim2.workItem.id, f.tx)
  await finishAgentWork(
    begun2.workItem.id,
    {
      resultStatus: 'Complete',
      completion: 100,
      notes: 'Second attempt complete.',
      commitHash: 'a2',
      testsSummary: 't2',
    },
    f.tx,
  )

  assert.equal(f.workItems.filter((w) => w.story_id === id).length, 2)
  assert.equal(f.runs.filter((r) => r.story_id === id).length, 2)
  const done = f.workItems.filter((w) => w.story_id === id)
  assert.ok(done.every((w) => w.state === 'Done'))
  const finalStory = f.stories.find((s) => s.id === id)!
  assert.equal(finalStory.status, 'Complete')
})

test('claim returns the authoritative story specification', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f, {
    goal: 'Deliver WhatsApp receipts.',
    preconditions: 'Provider webhook approved.',
    architectBrief: 'Reuse the media seam; keep engine generic.',
    contextRefs: 'db/migrations/021_*.sql',
    acceptanceCriteria: 'Idempotent receipt recorded.',
    postconditions: 'Notes preserved.',
  })
  await setStoryboardStatus(id, 'Ready', f.tx)

  const claim = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim)
  assert.equal(claim.story.id, id)
  assert.equal(claim.story.goal, 'Deliver WhatsApp receipts.')
  assert.equal(claim.story.preconditions, 'Provider webhook approved.')
  assert.equal(claim.story.architectBrief, 'Reuse the media seam; keep engine generic.')
  assert.equal(claim.story.contextRefs, 'db/migrations/021_*.sql')
  assert.equal(claim.story.acceptanceCriteria, 'Idempotent receipt recorded.')
  assert.equal(claim.story.postconditions, 'Notes preserved.')
})

test('claim refuses cleanly when another item is already Running', async () => {
  const f = new FakeQueueDb()
  await seedStory(f, { id: 'R-1' })
  await seedStory(f, { id: 'R-2' })
  f.enqueue('R-1')
  f.enqueue('R-2')

  const first = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(first)
  await beginAgentWorkRun(first.workItem.id, f.tx)
  assert.equal(first.workItem.state, 'Claimed')

  const second = await claimNextAgentWork('worker-2', f.runner)
  assert.equal(second, null)
})

test('getAgentWorkItem returns null for unknown ids', async () => {
  const f = new FakeQueueDb()
  const item = await getAgentWorkItem('nope', f.tx)
  assert.equal(item, null)
})

// ---------------------------------------------------------------------------
// Run telemetry + lifecycle (migration 026) — progress persistence, heartbeat,
// narrative accumulation, terminal outcomes (finish/fail/cancel), stale
// recovery, and queue unblocking. Deterministic clock via the fake's `now`.
// ---------------------------------------------------------------------------

async function seedRunningRun(
  f: FakeQueueDb,
  storyId = 'ENG-04',
): Promise<{ workItemId: string; runId: string }> {
  await setStoryboardStatus(storyId, 'Ready', f.tx)
  const claim = await claimNextAgentWork('worker-1', f.runner)
  assert.ok(claim)
  const begun = await beginAgentWorkRun(claim.workItem.id, f.tx)
  return { workItemId: begun.workItem.id, runId: begun.workItem.storyRunId! }
}

test('progress updates persist completion, append readable notes, and refresh the heartbeat', async () => {
  const f = new FakeQueueDb()
  await seedStory(f, { id: 'P-1' })
  const { workItemId } = await seedRunningRun(f, 'P-1')

  const first = await updateAgentWorkProgress(
    workItemId,
    { completion: 15, note: 'inspected existing map component and Google Maps loader' },
    f.tx,
  )
  f.now = '2026-08-21T12:30:00Z'
  const second = await updateAgentWorkProgress(
    workItemId,
    { completion: 50, note: 'replaced legacy marker construction with AdvancedMarkerElement' },
    f.tx,
  )
  f.now = '2026-08-21T13:00:00Z'
  const third = await updateAgentWorkProgress(
    workItemId,
    { completion: 80, testsSummary: 'targeted map tests: 8/8 passed' },
    f.tx,
  )

  assert.equal(first.run.completion, 15)
  assert.equal(second.run.completion, 50)
  assert.equal(third.run.completion, 80)

  const run = f.runs.find((r) => r.id === third.run.id)!
  assert.ok(run.notes!.includes('inspected existing map component'))
  assert.ok(run.notes!.includes('replaced legacy marker construction'))
  assert.ok(
    run.notes!.indexOf('inspected') < run.notes!.indexOf('replaced'),
    'notes accumulate in order',
  )
  assert.equal(run.tests_summary, 'targeted map tests: 8/8 passed')

  const item = f.workItems.find((w) => w.id === workItemId)!
  assert.equal(item.updated_at, '2026-08-21T13:00:00Z', 'heartbeat refreshed')
})

test('progress on an item that is not Running is rejected', async () => {
  const f = new FakeQueueDb()
  await seedStory(f, { id: 'P-2' })
  const { workItemId } = await seedRunningRun(f, 'P-2')
  await finishAgentWork(
    workItemId,
    {
      resultStatus: 'Partial',
      completion: 60,
      notes: 'Scaffold done.',
      commitHash: null,
      testsSummary: null,
    },
    f.tx,
  )

  await assert.rejects(
    updateAgentWorkProgress(workItemId, { completion: 80 }, f.tx),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'conflict',
  )
})


test('finish appends the final narrative to the accumulated progress notes', async () => {
  const f = new FakeQueueDb()
  await seedStory(f, { id: 'P-3' })
  const { workItemId } = await seedRunningRun(f, 'P-3')

  await updateAgentWorkProgress(
    workItemId,
    { completion: 80, note: 'full test suite running' },
    f.tx,
  )
  const finished = await finishAgentWork(
    workItemId,
    {
      resultStatus: 'Complete',
      completion: 100,
      notes: 'TypeScript passed; Next build passed; commit created.',
      commitHash: 'abc123',
      testsSummary: 'pnpm test: 124/124 passed; tsc passed; next build passed',
    },
    f.tx,
  )
  assert.equal(finished.workItem.state, 'Done')
  assert.ok(finished.workItem.finishedAt)

  const run = f.runs.find((r) => r.id === finished.workItem.storyRunId)!
  assert.ok(run.notes!.includes('full test suite running'), 'progress narrative preserved')
  assert.ok(run.notes!.includes('commit created.'), 'final summary appended')
  assert.equal(run.commit_hash, 'abc123')
  assert.equal(run.tests_summary, 'pnpm test: 124/124 passed; tsc passed; next build passed')
  assert.equal(run.result_status, 'Complete')
  assert.equal(run.ended_at, f.now)
})

test('failAgentWork terminates the run as Failed and the story as Failed', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f, { id: 'P-4' })
  const { workItemId } = await seedRunningRun(f, 'P-4')
  await updateAgentWorkProgress(workItemId, { completion: 40, note: 'mid-implementation' }, f.tx)

  const failed = await failAgentWork(
    workItemId,
    'worker process terminated unexpectedly',
    { completion: 40 },
    f.tx,
  )
  assert.equal(failed.state, 'Error')
  assert.ok(failed.finishedAt)
  assert.equal(failed.errorText, 'worker process terminated unexpectedly')

  const run = f.runs.find((r) => r.id === failed.storyRunId)!
  assert.equal(run.result_status, 'Failed')
  assert.equal(run.ended_at, f.now)
  assert.equal(run.completion, 40)
  assert.ok(run.notes!.includes('worker process terminated unexpectedly'))

  const story = f.stories.find((s) => s.id === id)!
  assert.equal(story.status, 'Failed')
})

test('cancelAgentWork terminates the run as Cancelled and the story as Hold', async () => {

test('stale Running work is listed, recovered to terminal, and the queue unblocks', async () => {
  const f = new FakeQueueDb()
  await seedStory(f, { id: 'S-1' })
  await seedStory(f, { id: 'S-2' })
  const { workItemId } = await seedRunningRun(f, 'S-1')
  f.enqueue('S-2', 50)
  await setStoryboardStatus('S-2', 'Ready', f.tx)

  // Fresh (heartbeated recently): not stale.
  assert.equal((await listStaleAgentWork(60, f.tx)).length, 0)

  // The worker dies: no heartbeat for 90 minutes.
  f.now = '2026-08-21T13:30:00Z'
  const stale = await listStaleAgentWork(60, f.tx)
  assert.equal(stale.length, 1)
  assert.equal(stale[0].id, workItemId)

  // Claim is still refused while the stale item is Running.
  assert.equal(await claimNextAgentWork('worker-2', f.runner), null)

  const recovered = await recoverStaleAgentWork(60, f.tx)
  assert.equal(recovered.length, 1)
  assert.equal(recovered[0].state, 'Error')
  assert.ok(recovered[0].errorText?.startsWith('stale: no heartbeat since'))

  const run = f.runs.find((r) => r.id === recovered[0].storyRunId)!
  assert.equal(run.result_status, 'Failed')
  assert.equal(run.ended_at, f.now)
  assert.ok(run.notes!.includes('marked stale'))

  const story = f.stories.find((s) => s.id === 'S-1')!
  assert.equal(story.status, 'Failed')

  // Queue unblocked: the next claim takes the Ready S-2.
  const claim = await claimNextAgentWork('worker-2', f.runner)
  assert.ok(claim)
  assert.equal(claim.workItem.storyId, 'S-2')
})

test('recoverStaleAgentWork does not touch fresh Running work', async () => {
  const f = new FakeQueueDb()
  const id = await seedStory(f, { id: 'S-3' })
  const { workItemId } = await seedRunningRun(f, 'S-3')
  await updateAgentWorkProgress(workItemId, { completion: 20, note: 'still alive' }, f.tx)

  const recovered = await recoverStaleAgentWork(60, f.tx)
  assert.equal(recovered.length, 0)

  const item = f.workItems.find((w) => w.id === workItemId)!
  assert.equal(item.state, 'Running')
  const story = f.stories.find((s) => s.id === id)!
  assert.equal(story.status, 'In Progress')
})

test('getActiveAgentWorkItem returns the single active item', async () => {
  const f = new FakeQueueDb()
  await seedStory(f, { id: 'A-5' })
  const { workItemId } = await seedRunningRun(f, 'A-5')

  const active = await getActiveAgentWorkItem(f.tx)
  assert.ok(active)
  assert.equal(active.id, workItemId)
})

  const f = new FakeQueueDb()
  const id = await seedStory(f, { id: 'P-5' })
  const { workItemId } = await seedRunningRun(f, 'P-5')
  await updateAgentWorkProgress(
    workItemId,
    { completion: 30, note: 'approach selected' },
    f.tx,
  )

  const cancelled = await cancelAgentWork(
    workItemId,
    { note: 'superseded by architect decision' },
    f.tx,
  )
  assert.equal(cancelled.state, 'Cancelled')
  assert.ok(cancelled.finishedAt)

  const run = f.runs.find((r) => r.id === cancelled.storyRunId)!
  assert.equal(run.result_status, 'Cancelled')
  assert.equal(run.ended_at, f.now)
  assert.equal(run.completion, 30, 'completion preserved')
  assert.ok(run.notes!.includes('approach selected'), 'progress narrative preserved')
  assert.ok(run.notes!.includes('superseded by architect decision'), 'cancellation explained')

  const story = f.stories.find((s) => s.id === id)!
  assert.equal(story.status, 'Hold')
})
