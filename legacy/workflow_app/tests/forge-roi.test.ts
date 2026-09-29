import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  ROI_UNIT,
  ROI_UNRECORDED_KIND,
  describeRoiRow,
  summarizeRoi,
  type RoiAttempt,
} from '@/lib/forge-roi'
import { listRoiAttempts, mapRoiAttempt } from '@/legacy/db/forge-roi'
import type { QueryExecutor, QueryRow } from '@/legacy/db/query-executor'

// ---------------------------------------------------------------------------
// ENG-FORGE-FACTORY-01 PHASE 4 — the thin ROI rollup.
//
// The packet's acceptance is a fixture: "one finished cheap fix and one judgment feature show as two rows
// in the rollup". That is the first test here. The rest is the discipline around it: nothing may be
// dropped for having no kind, coverage must be visible, and no output may read as money.
// ---------------------------------------------------------------------------

const attempt = (over: Partial<RoiAttempt> = {}): RoiAttempt => ({
  kind: 'fix',
  modelPolicy: 'cheap',
  state: 'Done',
  startedAt: '2026-09-15T00:00:00.000Z',
  finishedAt: '2026-09-15T00:10:00.000Z',
  resultStatus: 'Complete',
  costWidgets: 10,
  ...over,
})

test('one finished cheap fix and one judgment feature are two rows', () => {
  const summary = summarizeRoi(
    [
      attempt(), // fix/cheap, 10 minutes, 10 widgets
      attempt({
        kind: 'feature',
        modelPolicy: 'judgment',
        startedAt: '2026-09-15T01:00:00.000Z',
        finishedAt: '2026-09-15T01:30:00.000Z',
        costWidgets: 300,
      }),
    ],
    7,
  )

  assert.equal(summary.rows.length, 2)
  const fix = summary.rows.find((row) => row.kind === 'fix')
  const feature = summary.rows.find((row) => row.kind === 'feature')
  assert.ok(fix && feature, 'both kinds must appear as their own row')
  assert.deepEqual([fix.policy, fix.attempts, fix.completed], ['cheap', 1, 1])
  assert.equal(fix.meanWallMinutes, 10)
  assert.equal(fix.costWidgets, 10)
  assert.equal(feature.policy, 'judgment')
  assert.equal(feature.meanWallMinutes, 30)
  assert.equal(feature.costWidgets, 300)
  assert.equal(summary.totals.attempts, 2)
  assert.equal(summary.totals.costWidgets, 310)
})

test('an attempt with no recorded kind is visible, not dropped', () => {
  const summary = summarizeRoi([attempt({ kind: null, modelPolicy: null })])
  assert.equal(summary.rows.length, 1)
  assert.equal(summary.rows[0].kind, ROI_UNRECORDED_KIND)
  assert.equal(summary.rows[0].policy, ROI_UNRECORDED_KIND)
})

test('failures count as attempts, and a missing cost is not a zero cost', () => {
  const summary = summarizeRoi([
    attempt(),
    attempt({ state: 'Error', costWidgets: null, finishedAt: '2026-09-15T00:20:00.000Z' }),
  ])
  const row = summary.rows[0]
  assert.equal(row.attempts, 2)
  assert.equal(row.completed, 1)
  assert.equal(row.failed, 1)
  assert.equal(row.costWidgets, 10, 'a null cost must not be summed as zero')
  assert.equal(row.costKnown, 1, 'and the coverage says how many actually reported one')
  assert.equal(summary.coverage.costKnown, 1)
  assert.equal(summary.coverage.attempts, 2)
})

test('wall time is averaged only over the attempts that have both ends', () => {
  const summary = summarizeRoi([
    attempt(),
    attempt({ startedAt: null, finishedAt: null }),
    attempt({ finishedAt: '2026-09-15T00:20:00.000Z' }),
  ])
  assert.equal(summary.rows[0].wallMinutesKnown, 2)
  assert.equal(summary.rows[0].meanWallMinutes, 15)
  assert.equal(summary.coverage.wallTimeKnown, 2)
})

test('rows are ordered deterministically: most attempts, then kind, then policy', () => {
  const summary = summarizeRoi([
    attempt({ kind: 'qa' }),
    attempt({ kind: 'fix' }),
    attempt({ kind: 'fix' }),
    attempt({ kind: 'fix', modelPolicy: 'judgment' }),
  ])
  assert.deepEqual(
    summary.rows.map((row) => `${row.kind}/${row.policy}`),
    ['fix/cheap', 'fix/judgment', 'qa/cheap'],
  )
})

test('the summary never reads as money', () => {
  const summary = summarizeRoi([attempt({ costWidgets: 1234.5 })])
  const serializedRows = JSON.stringify(summary.rows)
  assert.ok(!serializedRows.includes('$'), 'no currency symbol can appear in a rollup row')
  assert.match(summary.unit, /widgets/)
  assert.match(summary.unit, /not dollars/)
  // The note is the reason a reader can trust the number: cost_usd is a different quantity.
  assert.match(summary.note, /vendor-reported actuals/)
  assert.match(describeRoiRow(summary.rows[0]), /widgets/)
})

test('the mapper normalizes what the driver actually returns', () => {
  // `numeric` comes back as a string from this driver, and timestamps as Date objects: both are
  // normalized at the repository boundary so no caller has to know that.
  const mapped = mapRoiAttempt({
    kind: 'feature',
    model_policy: 'judgment',
    state: 'Done',
    started_at: new Date('2026-09-15T00:00:00.000Z'),
    finished_at: '2026-09-15T00:20:00.000Z',
    result_status: 'Complete',
    cost_widgets: '42.50',
  })
  assert.equal(mapped.costWidgets, 42.5)
  assert.equal(mapped.startedAt, '2026-09-15T00:00:00.000Z')
  assert.equal(mapped.finishedAt, '2026-09-15T00:20:00.000Z')
})

test('the read joins the run for cost and result, over finished items only', async () => {
  const sql: string[] = []
  const tx: QueryExecutor = (strings, ...values) => {
    sql.push(strings.reduce((acc, part, i) => acc + part + (i < values.length ? `$${i + 1}` : ''), '').replace(/\s+/g, ' '))
    return Promise.resolve([] as QueryRow[])
  }
  await listRoiAttempts(7, tx)
  assert.match(sql[0], /from agent_work_item w/)
  assert.match(sql[0], /left join storyboard_story_run r on r\.id = w\.story_run_id/)
  assert.match(sql[0], /w\.finished_at is not null/)
  assert.match(sql[0], /finished_at >= now\(\) - \(\$1 \|\| ' days'\)::interval/)
  // LEFT, not inner: an item whose run is missing still counts as an attempt (coverage reports the gap).
  assert.match(sql[0], /left join/)
  assert.ok(!/\binner join\b/.test(sql[0]))
})
