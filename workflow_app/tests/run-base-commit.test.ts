import { test } from 'node:test'
import assert from 'node:assert/strict'
import { join } from 'node:path'
import { tmpdir } from 'node:os'

import {
  listStoryRunBaseCommits,
  resetRunV6ColumnCache,
  setStoryRunBaseCommit,
} from '../../db/storyboard'
import { readWorkerCommitHash } from '../../lib/worker-workspace'
import { recordedScopeBase, storyScopeBase } from '../forge/story-scope-base'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// ENG-FORGE-START-BASE-01 — START CARRIES ITS OWN BASE.
//
// A lane records the commit HEAD stood on when it began; the scope gate reads that fact
// instead of deriving one. These lock both halves at their seams: the run-row writer and
// reader in db/storyboard.ts, and the recorded-over-derived preference in
// story-scope-base.ts. An unreadable base records none — never a guess.
// ---------------------------------------------------------------------------

const SHA_LATE = 'a'.repeat(40)
const SHA_EARLY = 'b'.repeat(40)
const SHA_DERIVED = 'c'.repeat(40)

type FakeRunRow = {
  id: string
  story_id: string
  base_commit_hash: string | null
  started_at: string
}

/** A minimal storyboard_story_run fake: the V6 column probe, the base write and the base read. */
class FakeRuns {
  rows: FakeRunRow[] = []

  tx: QueryExecutor = (strings, ...params) => {
    const sql = strings.join('?').replace(/\s+/g, ' ').trim().toLowerCase()

    if (sql.includes('information_schema.columns')) {
      return Promise.resolve([{ count: 1 }] as QueryRow[])
    }
    if (sql.startsWith('update storyboard_story_run')) {
      const [base, id] = params as [string | null, string]
      const row = this.rows.find((candidate) => candidate.id === id)
      if (row) row.base_commit_hash = base
      return Promise.resolve([] as QueryRow[])
    }
    if (sql.includes('select base_commit_hash')) {
      const [storyId] = params as [string]
      const found = this.rows
        .filter((row) => row.story_id === storyId && row.base_commit_hash != null)
        .sort((a, b) => (a.started_at < b.started_at ? 1 : -1))
        .map((row) => ({ base_commit_hash: row.base_commit_hash, started_at: row.started_at }))
      return Promise.resolve(found as QueryRow[])
    }
    throw new Error(`unexpected SQL in FakeRuns: ${sql}`)
  }
}

function fakeRuns(): FakeRuns {
  resetRunV6ColumnCache()
  const f = new FakeRuns()
  f.rows = [{ id: 'run-1', story_id: 'ENG-X', base_commit_hash: null, started_at: '2026-09-18T00:00:00Z' }]
  return f
}

test("start records the lane's base on the run row", async () => {
  const f = fakeRuns()
  await setStoryRunBaseCommit('run-1', SHA_LATE, f.tx)
  assert.deepEqual(await listStoryRunBaseCommits('ENG-X', f.tx), [SHA_LATE])
  assert.equal(f.rows[0]?.base_commit_hash, SHA_LATE)
})

test('an unreadable base is recorded as none', async () => {
  const f = fakeRuns()
  const read = await readWorkerCommitHash(join(tmpdir(), 'no-such-lane-checkout-xyz'))
  assert.equal(read, null, 'a failed git read is null, not a guessed SHA')
  await setStoryRunBaseCommit('run-1', read, f.tx)
  assert.deepEqual(await listStoryRunBaseCommits('ENG-X', f.tx), [])
  assert.equal(f.rows[0]?.base_commit_hash, null)
})

test('the scope check prefers the earliest recorded base', () => {
  // Newest-first, as every reader in this repo orders runs. The story base is the EARLIEST.
  const recorded = [SHA_LATE, SHA_EARLY]
  const derived = storyScopeBase([SHA_LATE], () => SHA_DERIVED)
  assert.equal(recordedScopeBase(recorded), SHA_EARLY)
  assert.equal(recordedScopeBase(recorded) ?? derived, SHA_EARLY)
})

test('with no recorded base the scope check falls back to the derived base', () => {
  const derived = storyScopeBase([SHA_LATE], () => SHA_DERIVED)
  assert.equal(recordedScopeBase([null, undefined]), null)
  assert.equal(recordedScopeBase([null]) ?? derived, SHA_DERIVED)
})
