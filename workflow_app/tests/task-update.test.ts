import { afterEach, test } from 'node:test'
import assert from 'node:assert/strict'

import { setDatabaseTestExecutor } from '../../db/client'
import type { QueryExecutor } from '../../db/query-executor'
import { updateTask } from '../../db/portal-writes'

// CATCH-UP task edit seam — focused unit proof for updateTask (title / detail /
// due / priority). The canonical task write service is validated atomically:
//   - blank title and out-of-range priority fail before any query runs
//   - the update touches exactly the editable columns and updates the row
//   - a missing task surfaces as a not-found conflict
// Uses the DB test executor hook; never touches a real database.

afterEach(() => setDatabaseTestExecutor(null))

type Captured = { sql: string; params: unknown[] }

function makeExecutor(
  sequences: Record<string, unknown>[][],
  captured: Captured[],
): QueryExecutor {
  let i = 0
  return async (strings, ...params) => {
    captured.push({ sql: strings.join('?'), params })
    const set = sequences[Math.min(i, sequences.length - 1)] ?? []
    i++
    return set
  }
}

test('updateTask: rejects a blank title before any query', async () => {
  const captured: Captured[] = []
  setDatabaseTestExecutor(makeExecutor([[]], captured))
  await assert.rejects(
    updateTask('task-1', {
      title: '   ',
      detail: null,
      dueAt: null,
      priority: 0,
    }),
    /Task title is required/,
  )
  assert.equal(captured.length, 0, 'no query issued for invalid title')
})

test('updateTask: rejects an out-of-range priority before any query', async () => {
  const captured: Captured[] = []
  setDatabaseTestExecutor(makeExecutor([[]], captured))
  await assert.rejects(
    updateTask('task-1', {
      title: 'Title',
      detail: null,
      dueAt: null,
      priority: 50000,
    }),
    /Task priority must be an integer between 0 and 32767/,
  )
  assert.equal(captured.length, 0, 'no query issued for invalid priority')
})

test('updateTask: updates title/detail/due/priority/taxonomy atomically and trims title', async () => {
  const captured: Captured[] = []
  setDatabaseTestExecutor(
    makeExecutor([[{ id: 'task-1' }]], captured),
  )
  await updateTask('task-1', {
    title: '  New title  ',
    detail: 'notes',
    dueAt: '2026-09-01T00:00:00.000Z',
    priority: 2,
    workstream: 'CORE',
    category: 'MARKETING',
  })
  assert.equal(captured.length, 1)
  assert.ok(/update task/i.test(captured[0].sql))
  assert.ok(/set title = \?/i.test(captured[0].sql))
  assert.ok(/detail = \?/i.test(captured[0].sql))
  assert.ok(/due_at = \?/i.test(captured[0].sql))
  assert.ok(/priority = \?/i.test(captured[0].sql))
  assert.ok(/workstream = /i.test(captured[0].sql))
  assert.ok(/category = /i.test(captured[0].sql))
  assert.ok(/updated_at = now\(\)/i.test(captured[0].sql))
  assert.ok(captured[0].params.includes('New title'), 'title is trimmed')
  assert.ok(captured[0].params.includes('notes'))
  assert.ok(captured[0].params.includes('2026-09-01T00:00:00.000Z'))
  assert.ok(captured[0].params.includes(2))
  assert.ok(captured[0].params.includes('CORE'))
  assert.ok(captured[0].params.includes('MARKETING'))
})

test('updateTask: throws not-found when no row is updated', async () => {
  const captured: Captured[] = []
  setDatabaseTestExecutor(makeExecutor([[]], captured))
  await assert.rejects(
    updateTask('missing', {
      title: 'Title',
      detail: null,
      dueAt: null,
      priority: 0,
    }),
    /Task not found/,
  )
  assert.equal(captured.length, 1)
})
