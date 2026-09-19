// ---------------------------------------------------------------------------
// FORGE-PARITY-CHECK-01, second half — the parallel-shape rule expressed in code.
//
// The rule lived ONLY in a database constraint, and only in PROD's copy of it. That
// is how two malformed `agent_work_item` rows came to exist in DEV (2026-09-10):
// `recordSplitChildAssignment` set `parallel_slot` and `split_assignment` without
// ever setting `parallel_group_id`, so the row came out half-grouped. PROD refused
// that shape; DEV accepted it, because DEV had no constraint at all.
//
// Two guards, one invariant, and this file keeps them honest:
//   * `enqueueAgentWorkCommand` refuses a slot without a group, a group without a
//     slot, and a slot that is not a positive integer — BEFORE any SQL runs.
//   * `recordSplitChildAssignment` writes the group itself, from the same source the
//     enqueue uses, so both writers set one complete tuple.
//
// Everything here runs against a fake executor: no database.
// ---------------------------------------------------------------------------

import { describe, it } from 'node:test'
import assert from 'node:assert/strict'

import { enqueueAgentWorkCommand } from '../../db/agent-work'
import { recordSplitChildAssignment } from '../../db/forge-split-children'
import type { QueryExecutor } from '../../db/query-executor'

type Row = Record<string, unknown>

/** Records every statement it is handed, so both the guard AND the SQL are visible. */
class RecordingDb {
  statements: Array<{ sql: string; params: unknown[] }> = []

  /** Rows to return for the next call (the enqueue's lookup/insert). */
  rows: Row[] = []

  tx: QueryExecutor = (strings, ...params) => {
    const sql = strings.reduce(
      (acc, s, i) => acc + s + (i < params.length ? `$${i + 1}` : ''),
      '',
    )
    this.statements.push({ sql: sql.replace(/\s+/g, ' ').trim(), params: params as unknown[] })
    return Promise.resolve(this.rows as never)
  }
}

const STORY = 'FORGE-SHAPE-GUARD-01'

function enqueue(db: RecordingDb, input: Record<string, unknown>) {
  return enqueueAgentWorkCommand({ storyId: STORY, ...input } as never, db.tx)
}

function ranAnySql(db: RecordingDb): boolean {
  return db.statements.length > 0
}

describe('enqueueAgentWorkCommand refuses a half-grouped row before it writes', () => {
  it('a slot with NO group is refused (the exact shape the two DEV rows had)', async () => {
    const db = new RecordingDb()
    await assert.rejects(
      () => enqueue(db, { parallelSlot: 3, splitAssignment: 'a' }),
      /parallelSlot 3 without parallelGroupId/,
    )
    assert.equal(ranAnySql(db), false, 'the refusal must happen before any query')
  })

  it('a group with NO slot is refused', async () => {
    const db = new RecordingDb()
    await assert.rejects(
      () => enqueue(db, { parallelGroupId: 'group-1' }),
      /parallelGroupId without parallelSlot/,
    )
    assert.equal(ranAnySql(db), false)
  })

  it('slot 0 and a non-integer slot are refused (the stored slot is 1-based)', async () => {
    const db = new RecordingDb()
    await assert.rejects(
      () => enqueue(db, { parallelGroupId: 'group-1', parallelSlot: 0 }),
      /must be a 1-based integer \(got 0\)/,
    )
    await assert.rejects(
      () => enqueue(db, { parallelGroupId: 'group-1', parallelSlot: 1.5 }),
      /must be a 1-based integer \(got 1\.5\)/,
    )
    assert.equal(ranAnySql(db), false)
  })

  it('a well-formed grouped call proceeds (the guard is not a blanket refusal)', async () => {
    const db = new RecordingDb()
    db.rows = [{ id: 'wi-1', story_id: STORY }]
    // splitAssignment is required for a parallel group (the constraint refuses a grouped row without
    // one), so a test that means to prove the guard is not a blanket refusal must supply it. This test
    // predated that requirement and had been failing in silence because nothing ran this suite.
    // A grouped row must also declare its size: the constraint refuses a parallel row whose slot has no
    // group size to be measured against, so the "well-formed" shape is group + slot + size + assignment.
    await enqueue(db, {
      parallelGroupId: 'group-1',
      parallelSlot: 2,
      parallelSize: 2,
      splitAssignment: 'unit A',
    })
    // The numeric UPPER bounds stay the database's business on purpose: the guard
    // must not refuse a value the constraint would accept.
    assert.ok(ranAnySql(db), 'a legal shape must reach SQL')
  })

  it('serial work (no group, no slot) is untouched', async () => {
    const db = new RecordingDb()
    db.rows = [{ id: 'wi-1', story_id: STORY }]
    await enqueue(db, {})
    assert.ok(ranAnySql(db))
  })
})

describe('recordSplitChildAssignment writes a complete tuple', () => {
  it('sets parallel_group_id alongside slot and assignment — the bug that created the bad rows', async () => {
    const db = new RecordingDb()
    await recordSplitChildAssignment('wi-1', { assignmentId: 'assign-a', index: 1, groupId: 'proc-9' }, db.tx)
    assert.equal(db.statements.length, 1)
    const { sql, params } = db.statements[0]
    assert.match(sql, /set split_assignment = \$1/i)
    assert.match(sql, /parallel_group_id = \$2/i, 'the group must be written by this statement')
    assert.match(sql, /parallel_slot = \$3/i)
    assert.deepEqual(
      params,
      ['assign-a', 'proc-9', 2, 'wi-1'],
      'slot is index + 1 (1-based), group from the caller, then the where id',
    )
  })

  it('refuses a missing group rather than writing a row PROD would reject', async () => {
    const db = new RecordingDb()
    await assert.rejects(
      () => recordSplitChildAssignment('wi-1', { assignmentId: 'assign-a', index: 0, groupId: '  ' }, db.tx),
      /groupId is required/,
    )
    assert.equal(ranAnySql(db), false)
  })

  it('refuses a blank assignment and a negative index', async () => {
    const db = new RecordingDb()
    await assert.rejects(
      () => recordSplitChildAssignment('wi-1', { assignmentId: '', index: 0, groupId: 'proc-9' }, db.tx),
      /assignmentId is required/,
    )
    await assert.rejects(
      () => recordSplitChildAssignment('wi-1', { assignmentId: 'a', index: -1, groupId: 'proc-9' }, db.tx),
      /must be a 0-based integer/,
    )
    assert.equal(ranAnySql(db), false)
  })
})
