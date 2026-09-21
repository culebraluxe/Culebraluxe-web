// ---------------------------------------------------------------------------
// ENG-FORGE-SPLIT-SHAPE-01 — a SPLIT child is written in the shape the engine can claim.
//
// `agent_work_item_parallel_shape_check` admits a grouped row only when it carries
// lane='smith', a parallel_size (2..3) and a non-empty split_assignment. split_assignment
// is the ONE clause that is not inert: lane/parallel_size were never written (so they
// evaluated NULL, which passes), but a null split_assignment evaluates FALSE and the
// grouped INSERT is refused 23514 on both DEV and PROD. The post-claim
// `recordSplitChildAssignment` UPDATE runs after the immediate CHECK has already judged the
// row, so `enqueueAgentWorkCommand` must carry the whole tuple at INSERT.
//
// Everything here runs against a fake executor: no database. The duplicate-slot refusal is
// asserted as the enqueue REJECTING a 23505 on a grouped row — the grouped path must never
// take the serial collapse fallback, which is what makes a sibling unclaimable.
// ---------------------------------------------------------------------------

import { it } from 'node:test'
import assert from 'node:assert/strict'

import { enqueueAgentWorkCommand } from '@/legacy/db/agent-work'
import type { QueryExecutor } from '@/legacy/db/query-executor'

type Row = Record<string, unknown>
type Response = Row[] | { error: Error }

/** A fake executor: statements are recorded, responses are consumed in order. */
class FakeDb {
  statements: Array<{ sql: string; params: unknown[] }> = []
  responses: Response[] = []

  tx: QueryExecutor = (strings, ...params) => {
    const sql = strings.reduce(
      (acc, s, i) => acc + s + (i < params.length ? `$${i + 1}` : ''),
      '',
    )
    this.statements.push({ sql: sql.replace(/\s+/g, ' ').trim(), params: params as unknown[] })
    const next = this.responses.shift() ?? []
    if (!Array.isArray(next)) return Promise.reject(next.error)
    return Promise.resolve(next as never)
  }
}

const STORY = 'FORGE-SPLIT-SHAPE-01'

function enqueue(db: FakeDb, input: Record<string, unknown>) {
  return enqueueAgentWorkCommand({ storyId: STORY, ...input } as never, db.tx)
}

function insertStatements(db: FakeDb) {
  return db.statements.filter((s) => /^insert into agent_work_item/i.test(s.sql))
}

it('split-child-shape:insert-writes-tuple writes lane, parallel_size and split_assignment at INSERT', async () => {
  const db = new FakeDb()
  db.responses = [[], [{ id: 'wi-1', story_id: STORY }]]
  await enqueue(db, {
    parallelGroupId: 'group-1',
    parallelSlot: 1,
    splitAssignment: 'assign-a',
    parallelSize: 2,
  })
  const insert = insertStatements(db)[0]
  assert.ok(insert, 'a grouped call must reach the INSERT (the lookup finds no sibling)')
  assert.match(insert.sql, /\blane\b/)
  assert.match(insert.sql, /\bparallel_size\b/)
  assert.match(insert.sql, /\bsplit_assignment\b/)
  assert.equal(insert.params[8], 'group-1', 'parallel_group_id')
  assert.equal(insert.params[9], 1, 'parallel_slot')
  assert.equal(insert.params[10], 2, 'parallel_size')
  assert.equal(insert.params[11], 'smith', 'lane is fixed to smith for a grouped row')
  assert.equal(insert.params[12], 'assign-a', 'split_assignment is the child own assignment')
})

it('split-child-shape:constraint-shape emits a tuple agent_work_item_parallel_shape_check accepts', async () => {
  const db = new FakeDb()
  db.responses = [[], [{ id: 'wi-1', story_id: STORY }]]
  await enqueue(db, {
    parallelGroupId: 'group-1',
    parallelSlot: 2,
    splitAssignment: 'assign-b',
    parallelSize: 2,
  })
  const p = insertStatements(db)[0].params
  const lane = p[11]
  const slot = p[9] as number
  const size = p[10] as number
  const assignment = p[12] as string
  assert.equal(lane, 'smith')
  assert.ok(Number.isInteger(slot) && slot >= 1 && slot <= 3, `slot in 1..3 (got ${String(slot)})`)
  assert.ok(Number.isInteger(size) && size >= 2 && size <= 3, `size in 2..3 (got ${String(size)})`)
  assert.ok(slot <= size, 'slot <= size')
  assert.equal(typeof assignment, 'string')
  assert.notEqual(assignment.trim(), '', 'split_assignment must be non-empty')
})

it('split-child-shape:siblings-distinct-slots puts two siblings in one group on distinct slots', async () => {
  const db = new FakeDb()
  db.responses = [
    [],
    [{ id: 'wi-1', story_id: STORY }],
    [],
    [{ id: 'wi-2', story_id: STORY }],
  ]
  await enqueue(db, {
    parallelGroupId: 'group-1',
    parallelSlot: 1,
    splitAssignment: 'assign-a',
    parallelSize: 2,
  })
  await enqueue(db, {
    parallelGroupId: 'group-1',
    parallelSlot: 2,
    splitAssignment: 'assign-b',
    parallelSize: 2,
  })
  const inserts = insertStatements(db)
  assert.equal(inserts.length, 2, 'each sibling gets its OWN row')
  assert.equal(inserts[0].params[8], 'group-1')
  assert.equal(inserts[1].params[8], 'group-1')
  assert.deepEqual([inserts[0].params[9], inserts[1].params[9]], [1, 2])
})

it('split-child-shape:duplicate-slot-refused rethrows a 23505 for a grouped row instead of collapsing siblings', async () => {
  const db = new FakeDb()
  const duplicate = Object.assign(
    new Error('duplicate key value violates unique constraint "agent_work_item_one_parallel_slot"'),
    { code: '23505' },
  )
  db.responses = [[], { error: duplicate }]
  await assert.rejects(
    () =>
      enqueue(db, {
        parallelGroupId: 'group-1',
        parallelSlot: 1,
        splitAssignment: 'assign-a',
        parallelSize: 2,
      }),
    'a duplicate slot must surface, never merge the second child onto the first',
  )
})

it('split-child-shape:serial-columns-null keeps every parallel column NULL for serial work', async () => {
  const db = new FakeDb()
  db.responses = [[], [{ id: 'wi-1', story_id: STORY }]]
  await enqueue(db, {})
  const p = insertStatements(db)[0].params
  assert.equal(p[8], null, 'parallel_group_id')
  assert.equal(p[9], null, 'parallel_slot')
  assert.equal(p[10], null, 'parallel_size')
  assert.equal(p[11], null, 'lane')
  assert.equal(p[12], null, 'split_assignment')
})

it('split-child-shape:two-unit-wave gives both siblings a non-empty assignment so both lanes can start', async () => {
  const db = new FakeDb()
  db.responses = [
    [],
    [{ id: 'wi-1', story_id: STORY }],
    [],
    [{ id: 'wi-2', story_id: STORY }],
  ]
  const first = await enqueue(db, {
    parallelGroupId: 'group-1',
    parallelSlot: 1,
    splitAssignment: 'assign-a',
    parallelSize: 2,
  })
  const second = await enqueue(db, {
    parallelGroupId: 'group-1',
    parallelSlot: 2,
    splitAssignment: 'assign-b',
    parallelSize: 2,
  })
  assert.ok(first.id, 'the first child has its own claimable row')
  assert.ok(second.id, 'the second child has its own claimable row')
  const inserts = insertStatements(db)
  assert.equal(inserts.length, 2)
  for (const insert of inserts) {
    assert.equal(insert.params[11], 'smith')
    assert.equal(insert.params[10], 2)
    assert.equal(typeof insert.params[12], 'string')
    assert.notEqual((insert.params[12] as string).trim(), '')
  }
})
