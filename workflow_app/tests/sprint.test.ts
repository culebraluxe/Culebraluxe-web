import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { QueryExecutor } from '../../db/query-executor'
import { PortalWriteError } from '../../lib/portal-write-error'
import {
  closeSprint,
  listSprintRollups,
  openSprint,
  sprintCloseRefusal,
  sprintIdForBatch,
  sprintNumberFromId,
} from '../../db/sprint'

// ---------------------------------------------------------------------------
// SPRINT PARENT (migration 187) — focused unit proof.
//
// Two things are worth pinning here and they are different things:
//
//   1. THE PURE RULES. `S<batch>` is the only spelling of the batch axis, and a sprint may not close
//      without saying what happened. The close rule is written so every branch is true or false and
//      NEVER NULL, because a CHECK constraint treats NULL as satisfied: the first version of the
//      database rule was `status <> 'Closed' or (closed_at is not null and outcome is not null)`,
//      which is NULL — and therefore passed — for exactly the write it exists to refuse. The
//      whitespace case below is that bug's fingerprint.
//   2. THE WRITE PATHS REFUSE BEFORE THEY WRITE. A refusal that still issues the update is not a
//      refusal, so these tests capture the SQL and assert the absent write.
//
// No database: the executor is injected, as the storyboard and accounting suites do.
// ---------------------------------------------------------------------------

type Captured = { sql: string; params: unknown[] }

function makeExecutor(
  sequences: Array<Array<Record<string, unknown>>>,
  captured: Captured[] = [],
): QueryExecutor {
  let i = 0
  return async (strings, ...params) => {
    captured.push({ sql: strings.join('?'), params })
    const set = sequences[Math.min(i, sequences.length - 1)] ?? []
    i++
    return set
  }
}

const normalize = (sql: string) => sql.replace(/\s+/g, ' ').trim().toLowerCase()

function wroteSprint(captured: Captured[], verb: 'insert' | 'update'): boolean {
  const prefix = verb === 'insert' ? 'insert into storyboard_sprint' : 'update storyboard_sprint'
  return captured.some((c) => normalize(c.sql).startsWith(prefix))
}

/** How many statements the run issued at all — so "nothing was written" cannot pass by not running. */
function statements(captured: Captured[]): number {
  return captured.length
}

const rollupRow = (over: Record<string, unknown> = {}): Record<string, unknown> => ({
  id: 'S92',
  number: 92,
  title: 'Sprint 92',
  theme: null,
  goal: 'finish the release story',
  status: 'Active',
  owner: null,
  started_at: null,
  target_end_at: null,
  closed_at: null,
  outcome: null,
  notes: null,
  stories: 18,
  stories_complete: 10,
  stories_open: 8,
  stories_held: 0,
  percent_complete: 55.6,
  first_completed_at: null,
  last_completed_at: null,
  ...over,
})

// --- 1. The one spelling of the batch axis ---------------------------------

test('sprintIdForBatch: a batch and its sprint are the same number, spelled once', () => {
  assert.equal(sprintIdForBatch(92), 'S92')
  assert.equal(sprintIdForBatch(0), 'S0')
  assert.equal(sprintIdForBatch(100), 'S100')
})

test('sprintIdForBatch: an absent batch stays absent, never invented into a sprint', () => {
  // 283 stories carry a null batch. They belong to no sprint, and `Snull` is not a sprint.
  assert.equal(sprintIdForBatch(null), null)
  assert.equal(sprintIdForBatch(undefined), null)
  assert.equal(sprintIdForBatch(Number.NaN), null)
})

test('sprintNumberFromId: reads the number back, and refuses anything that is not S<digits>', () => {
  assert.equal(sprintNumberFromId('S92'), 92)
  assert.equal(sprintNumberFromId('S0'), 0)
  assert.equal(sprintNumberFromId('s92'), null, 'the spelling is exact')
  assert.equal(sprintNumberFromId('S'), null)
  assert.equal(sprintNumberFromId('S92b'), null)
  assert.equal(sprintNumberFromId(null), null)
  assert.equal(sprintNumberFromId(undefined), null)
})

test('sprintIdForBatch and sprintNumberFromId round-trip', () => {
  for (const batch of [0, 1, 91, 100]) {
    assert.equal(sprintNumberFromId(sprintIdForBatch(batch)), batch)
  }
})

// --- 2. The close rule, including the NULL-logic fingerprint ----------------

test('sprintCloseRefusal: a closed sprint with an outcome and a close time is allowed', () => {
  assert.equal(
    sprintCloseRefusal({ status: 'Closed', closedAt: '2026-09-17T18:00:00.000Z', outcome: 'shipped' }),
    null,
  )
})

test('sprintCloseRefusal: closed with no close time says which field is missing', () => {
  const refusal = sprintCloseRefusal({ status: 'Closed', closedAt: null, outcome: 'shipped' })
  assert.match(String(refusal), /closed_at/)
})

test('sprintCloseRefusal: closed with no outcome says what is missing', () => {
  const refusal = sprintCloseRefusal({
    status: 'Closed',
    closedAt: '2026-09-17T18:00:00.000Z',
    outcome: null,
  })
  assert.match(String(refusal), /outcome/)
})

test('sprintCloseRefusal: a WHITESPACE-ONLY outcome is refused, not treated as an outcome', () => {
  // The fingerprint of the bug that shipped first: an outcome that is present-but-empty satisfied
  // the constraint's first version, because `' ' is not null` is true and NULL never failed a CHECK.
  for (const outcome of ['', ' ', '\n', '\t  ']) {
    const refusal = sprintCloseRefusal({
      status: 'Closed',
      closedAt: '2026-09-17T18:00:00.000Z',
      outcome,
    })
    assert.match(String(refusal), /outcome/, `outcome ${JSON.stringify(outcome)} must be refused`)
  }
})

test('sprintCloseRefusal: an open sprint is not a close and needs no outcome', () => {
  for (const status of ['Planned', 'Active', 'Closing', 'Cancelled']) {
    assert.equal(sprintCloseRefusal({ status, closedAt: null, outcome: null }), null)
  }
})

// --- 3. Reading the rollup --------------------------------------------------

test('listSprintRollups: returns null when the table is not there, rather than an empty board', async () => {
  const exec = makeExecutor([[{ ready: false }]])
  assert.equal(await listSprintRollups(exec), null)
})

test('listSprintRollups: a sprint with no stories reports null percent, not 0%', async () => {
  const exec = makeExecutor([
    [{ ready: true }],
    [
      rollupRow({
        number: 100,
        id: 'S100',
        stories: 0,
        stories_complete: 0,
        stories_open: 0,
        percent_complete: null,
      }),
    ],
  ])
  const rows = (await listSprintRollups(exec)) ?? []
  assert.equal(rows[0]?.stories, 0)
  assert.equal(rows[0]?.percentComplete, null, '0/0 is not 0%: it is unmeasured')
})

// --- 4. The write paths refuse before they write -----------------------------

async function refusalFrom(run: () => Promise<unknown>): Promise<PortalWriteError> {
  try {
    await run()
  } catch (error) {
    assert.ok(error instanceof PortalWriteError, `expected a PortalWriteError, got ${String(error)}`)
    return error
  }
  throw new Error('expected a refusal, and there was none')
}

test('openSprint: a sprint that already exists is refused, and nothing is written over it', async () => {
  const captured: Captured[] = []
  const exec = makeExecutor([[{ id: 'S92' }]], captured)
  const error = await refusalFrom(() => openSprint({ number: 92, title: 'Sprint 92' }, exec))
  assert.equal(error.code, 'conflict')
  assert.match(error.message, /S92/)
  assert.equal(wroteSprint(captured, 'insert'), false, 'a refusal must not insert')
  assert.equal(statements(captured), 1, 'the refusal read once and wrote nothing')
})

test('openSprint: a sprint with no title is refused without touching the database', async () => {
  const captured: Captured[] = []
  const exec = makeExecutor([[]], captured)
  const error = await refusalFrom(() => openSprint({ number: 101, title: '   ' }, exec))
  assert.equal(error.code, 'validation')
  assert.equal(captured.length, 0, 'the refusal is decidable from the input alone')
})

test('openSprint: a valid sprint is written, Active from the start, and read back', async () => {
  const captured: Captured[] = []
  const exec = makeExecutor(
    [[], [], [rollupRow({ number: 101, id: 'S101', title: 'Sprint 101', status: 'Active' })]],
    captured,
  )
  const created = await openSprint(
    { number: 101, title: 'Sprint 101', goal: 'finish the story' },
    exec,
  )
  assert.equal(created.id, 'S101')
  assert.equal(created.status, 'Active')
  assert.equal(wroteSprint(captured, 'insert'), true)
})

test('closeSprint: closing with an empty outcome is refused, and the update is NOT issued', async () => {
  const captured: Captured[] = []
  const exec = makeExecutor([[rollupRow({ status: 'Active' })]], captured)
  const error = await refusalFrom(() => closeSprint(92, '   ', exec))
  assert.equal(error.code, 'validation')
  assert.match(error.message, /outcome/)
  assert.equal(wroteSprint(captured, 'update'), false, 'a refused close must not write Closed')
})

test('closeSprint: closing for real records status, time and outcome together', async () => {
  const captured: Captured[] = []
  const exec = makeExecutor(
    [
      [rollupRow({ status: 'Active' })],
      [],
      [rollupRow({ status: 'Closed', outcome: 'shipped 23 of 61', closed_at: '2026-09-17T18:00:00.000Z' })],
    ],
    captured,
  )
  const closed = await closeSprint(92, 'shipped 23 of 61', exec)
  assert.equal(closed.status, 'Closed')
  assert.equal(closed.outcome, 'shipped 23 of 61')
  assert.equal(wroteSprint(captured, 'update'), true)
})

test('closeSprint: a sprint that does not exist is not-found, not a silent no-op', async () => {
  const captured: Captured[] = []
  const exec = makeExecutor([[]], captured)
  const error = await refusalFrom(() => closeSprint(999, 'nothing to close', exec))
  assert.equal(error.code, 'not-found')
  assert.equal(wroteSprint(captured, 'update'), false)
})

