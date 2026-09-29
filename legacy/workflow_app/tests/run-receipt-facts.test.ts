import assert from 'node:assert/strict'
import test from 'node:test'

import {
  SPEND_SOURCES,
  NO_COMMIT_SHA,
  isCommitSha,
  normalizeCommitHash,
  resolveRunSpendSource,
  summarizeRunSpend,
  finishStoryRun,
  listStoryCommitHashes,
} from '@/legacy/db/storyboard'
import { readForgeGenerationFacts } from '@/legacy/db/forge-run'
import { receiptShaFromArtifact, resolveReceiptSha } from '@/legacy/db/forge-artifact'
import type { QueryExecutor, QueryRow } from '@/legacy/db/query-executor'

// ENG-FORGE-RECEIPT-COLUMNS-01 — a run receipt carries its own facts.
//
// The sha is in the column (commit_hash), the cost source is a closed set written on every
// completed run, and a spend total states the coverage it was computed from. A receipt whose
// sha exists only inside its notes is a NAMED defect. Pure + fake-executor only: no live
// database is read.

const CANDIDATE = 'a1b2c3d4e5f60718293a4b5c6d7e8f9012345678'

type Call = { sql: string; params: unknown[] }

/** The bound parameter that follows the first occurrence of `marker` in a captured query. */
function paramAfter(call: Call, marker: string): unknown {
  const at = call.sql.indexOf(marker)
  if (at < 0) return undefined
  const index = (call.sql.slice(0, at).match(/\?/g) ?? []).length
  return call.params[index]
}

function finishHarness(): { calls: Call[]; q: QueryExecutor } {
  const calls: Call[] = []
  const runRow = {
    id: 'run-1',
    story_id: 'STORY-1',
    started_at: '2026-09-18T10:00:00Z',
    ended_at: null,
    result_status: null,
    run_type: null,
    agent_runtime: null,
    completion: null,
    notes: null,
    commit_hash: null,
    tests_summary: null,
    execution_environment: null,
    model_used: null,
    cost_usd: null,
    cost_widgets: null,
    cost_source: null,
    created_at: '2026-09-18T10:00:00Z',
    updated_at: '2026-09-18T10:00:00Z',
  }
  const storyRow = {
    id: 'STORY-1',
    workstream: 'ENGINEERING',
    title: 'receipt facts',
    priority: 'High',
    status: 'Complete',
    notes: '',
    completion: 100,
    rollup: false,
    created_at: '2026-09-18T10:00:00Z',
    updated_at: '2026-09-18T10:00:00Z',
  }
  const q: QueryExecutor = (strings, ...params) => {
    const sql = strings.join('?')
    calls.push({ sql, params })
    if (sql.includes('information_schema.columns')) {
      // Model the legacy run shape so finishStoryRun takes one literal path.
      return Promise.resolve([{ count: 0 }] as QueryRow[])
    }
    if (sql.includes('update storyboard_story_run')) {
      return Promise.resolve([runRow] as QueryRow[])
    }
    if (sql.includes('update storyboard_story')) {
      return Promise.resolve([storyRow] as QueryRow[])
    }
    return Promise.resolve([] as QueryRow[])
  }
  return { calls, q }
}

function finishInput(commitHash: string | null) {
  return {
    resultStatus: 'Complete',
    completion: 100,
    notes: 'finished',
    commitHash,
    testsSummary: null,
  }
}

test('completed-run-carries-candidate-sha', async () => {
  const { calls, q } = finishHarness()
  await finishStoryRun('run-1', finishInput(CANDIDATE), q)
  const update = calls.find((c) => c.sql.includes('set ended_at = now()'))
  assert.ok(update, 'the run update ran')
  const written = paramAfter(update!, 'commit_hash =')
  assert.equal(written, CANDIDATE, 'the candidate sha lands in the commit_hash column')
  assert.ok(isCommitSha(written), 'the column holds a real sha')
})

test('no-commit-is-explicit-not-empty', async () => {
  const { calls, q } = finishHarness()
  await finishStoryRun('run-1', finishInput(null), q)
  const update = calls.find((c) => c.sql.includes('set ended_at = now()'))
  const written = paramAfter(update!, 'commit_hash =')
  assert.equal(written, NO_COMMIT_SHA, 'a run that produced no commit says so')
  assert.notEqual(written, null, 'the column is never left empty')
  assert.ok(!isCommitSha(NO_COMMIT_SHA), 'the marker is not a sha')
  assert.equal(normalizeCommitHash('   '), NO_COMMIT_SHA)

  // No reader may resolve the marker as a sha.
  const rows: QueryRow[] = [
    { commit_hash: CANDIDATE, started_at: '2026-09-18T10:00:00Z' },
    { commit_hash: NO_COMMIT_SHA, started_at: '2026-09-18T09:00:00Z' },
    { commit_hash: null, started_at: '2026-09-18T08:00:00Z' },
  ]
  const fake: QueryExecutor = () => Promise.resolve(rows)
  assert.deepEqual(await listStoryCommitHashes('STORY-1', fake), [CANDIDATE])
  const facts = await readForgeGenerationFacts('STORY-1', 20, fake)
  assert.deepEqual(facts.candidateShas, [CANDIDATE])
})

test('unrecorded-is-written-not-null', async () => {
  const { calls, q } = finishHarness()
  await finishStoryRun('run-1', finishInput(null), q)
  const costCall = calls.find((c) => c.sql.includes('cost_source ='))
  assert.ok(costCall, 'the spend source is written on a completed run')
  const source = paramAfter(costCall!, 'cost_source =')
  assert.equal(source, 'none', 'absence is the recorded fact none')
  assert.notEqual(source, null, 'unrecorded is WRITTEN, never NULL')
})

test('cost-source-is-closed-set', () => {
  assert.deepEqual([...SPEND_SOURCES], ['vendor', 'widgets', 'none'])
  assert.equal(resolveRunSpendSource({ costSource: 'vendor', costUsd: 1 }), 'vendor')
  assert.equal(resolveRunSpendSource({ costUsd: 12.5 }), 'vendor')
  assert.equal(resolveRunSpendSource({ costWidgets: 3.5 }), 'widgets')
  assert.equal(resolveRunSpendSource({}), 'none')
  const bogus = resolveRunSpendSource({ costSource: 'vendor:acme' })
  assert.ok((SPEND_SOURCES as readonly string[]).includes(bogus), 'always a member of the closed set')
})

test('spend-total-states-coverage', () => {
  const summary = summarizeRunSpend([
    { costUsd: 2, costSource: 'vendor' },
    { costWidgets: 3, costSource: 'widgets' },
    { costSource: 'none' },
    { costSource: null },
  ])
  assert.equal(summary.runs, 4)
  assert.equal(summary.coveredRuns, 2, 'the total states how many runs it covers')
  assert.equal(summary.unrecordedRuns, 2)
  assert.equal(summary.totalUsd, 2)
  assert.equal(summary.totalWidgets, 3)
})

test('receipt-sha-is-in-column-not-notes', () => {
  const resolved = resolveReceiptSha({ sha: CANDIDATE, notes: 'qa pass' })
  assert.equal(resolved.source, 'column')
  assert.equal(resolved.sha, CANDIDATE.toLowerCase())
  assert.equal(resolved.defect, null)
  const row = receiptShaFromArtifact({ sha: CANDIDATE, summary: 'qa pass' })
  assert.equal(row.source, 'column')
  assert.equal(row.sha, CANDIDATE.toLowerCase())
})

test('notes-only-sha-is-named-defect', () => {
  const resolved = resolveReceiptSha({ sha: null, notes: `verified candidate ${CANDIDATE}` })
  assert.equal(resolved.source, 'notes')
  assert.equal(resolved.sha, null)
  assert.match(String(resolved.defect), /exists only in notes/)
  assert.match(String(resolved.defect), new RegExp(CANDIDATE, 'i'))
  const absent = resolveReceiptSha({ sha: null, notes: 'no sha here' })
  assert.equal(absent.source, 'absent')
  assert.equal(absent.defect, null)
})
