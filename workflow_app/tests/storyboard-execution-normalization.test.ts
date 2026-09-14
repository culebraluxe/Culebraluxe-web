// ---------------------------------------------------------------------------
// REPOSITORY BOUNDARY NORMALIZATION — the projection that crashed the TECH cockpit.
//
// `listStoryExecutionSummaries` declared `latestRunAt: string | null` while handing back whatever
// the driver returned — a `Date`. A caller that trusted the type and called a string method on it
// (`.localeCompare` while sorting the run-history signpost) threw inside the server component
// render, so /portal/tech answered a React server-render error (digest 3001149126, React #441) to
// a signed-in operator while the unauthenticated redirect stayed perfectly healthy.
//
// This test feeds the REAL driver shape (a `Date` on the row, not a string) through the function's
// injected executor, which is what the repository contract requires: above `db/`, a driver value
// must never appear.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { listStoryExecutionSummaries } from '../../db/storyboard'
import type { QueryExecutor } from '../../db/query-executor'

/** A tagged-template executor that answers the two queries this projection issues. */
function fakeExecutor(rows: { work: unknown[]; runs: unknown[] }): QueryExecutor {
  return ((strings: TemplateStringsArray) => {
    const text = strings.join(' ')
    return Promise.resolve(text.includes('agent_work_item') ? rows.work : rows.runs)
  }) as unknown as QueryExecutor
}

test('NORMALIZE: a driver Date becomes an ISO string at the boundary', async () => {
  const startedAt = new Date('2026-09-14T01:30:45.242Z')
  const summaries = await listStoryExecutionSummaries(
    fakeExecutor({
      work: [],
      runs: [{ story_id: 'ENG-FORGE-TURN-VISIBILITY-01', result_status: 'Completed', started_at: startedAt }],
    }),
  )

  assert.equal(summaries.length, 1)
  const summary = summaries[0]
  assert.equal(typeof summary.latestRunAt, 'string', 'latestRunAt must be a string above the repository')
  assert.equal(summary.latestRunAt, '2026-09-14T01:30:45.242Z')
  assert.equal(summary.latestRunResult, 'Completed')
})

test('NORMALIZE: the summary supports the exact string operations its type promises', async () => {
  const summaries = await listStoryExecutionSummaries(
    fakeExecutor({
      work: [],
      runs: [
        { story_id: 'A', result_status: 'pass', started_at: new Date('2026-09-13T00:00:00.000Z') },
        { story_id: 'B', result_status: 'pass', started_at: new Date('2026-09-14T00:00:00.000Z') },
      ],
    }),
  )

  // The exact call that threw in production: sorting newest-first in the cockpit page.
  const sorted = [...summaries].sort((a, b) => (b.latestRunAt ?? '').localeCompare(a.latestRunAt ?? ''))
  assert.deepEqual(sorted.map((s) => s.storyId), ['B', 'A'])
  // And the slice the screen renders must not care whether a driver Date is underneath.
  assert.equal(sorted[0].latestRunAt?.slice(0, 10), '2026-09-14')
})

test('NORMALIZE: a missing timestamp stays null rather than becoming "null" or an epoch', async () => {
  const summaries = await listStoryExecutionSummaries(
    fakeExecutor({ work: [], runs: [{ story_id: 'C', result_status: null, started_at: null }] }),
  )
  assert.equal(summaries[0].latestRunAt, null)
  assert.equal(summaries[0].latestRunResult, null)
})
