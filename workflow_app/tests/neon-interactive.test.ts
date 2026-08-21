import { test } from 'node:test'
import assert from 'node:assert/strict'

import { flatten, makeQueryFn } from '../../lib/neon-interactive'

// ---------------------------------------------------------------------------
// TUNIT harvest mechanism #2 — Neon interactive transaction surface.
//
// The WebSocket-Pool atomicity itself is proven live against DEV (CRM-14H/J);
// the durable part is the tagged-template surface this adapter presents:
//
//   - nested-fragment flattening with correct $N parameter numbering
//     (the same class of gotcha as the Neon `?column?` driver issue),
//   - the lazy thenable that memoizes a query's run promise so concurrent
//     consumers share one execution.
//
// All assertions run on in-memory fakes; no DB/Neon required.
// ---------------------------------------------------------------------------

test('flatten numbers parameters correctly in a simple query', () => {
  const { text, params } = flatten(
    ['select * from t where a = ', ' and b = ', ''],
    [10, 'x'],
  )
  assert.equal(text, 'select * from t where a = $1 and b = $2')
  assert.deepEqual(params, [10, 'x'])
})

test('flatten treats a nested fragment as inline SQL, not a parameter', () => {
  const fragment = { strings: ['c = ', ''], values: [99] }
  const { text, params } = flatten(['select * from t where ', ''], [fragment])
  assert.equal(text, 'select * from t where c = $1')
  assert.deepEqual(params, [99])
})

test('flatten recurses through multiple nested fragments preserving order', () => {
  const inner = { strings: ['x = ', ' and y = ', ''], values: [1, 2] }
  const { text, params } = flatten(
    ['select * from t where ', ' and z = ', ''],
    [inner, 3],
  )
  assert.equal(text, 'select * from t where x = $1 and y = $2 and z = $3')
  assert.deepEqual(params, [1, 2, 3])
})

test('flatten does not invent a parameter for a trailing empty segment', () => {
  const { text, params } = flatten(['select 1 where a = ', ''], [7])
  assert.equal(text, 'select 1 where a = $1')
  assert.deepEqual(params, [7])
})

test('makeQueryFn returns a lazy thenable that memoizes one run', async () => {
  let runs = 0
  const q = makeQueryFn(async (text, params) => {
    runs += 1
    assert.equal(text, 'select 1 where a = $1')
    assert.deepEqual(params, [5])
    return [{ id: 1 }]
  })

  const a = q`select 1 where a = ${5}`
  const [ra, rb] = await Promise.all([a, a])
  assert.equal(runs, 1, 'concurrent consumers share one memoized run')
  assert.deepEqual(ra, [{ id: 1 }])
  assert.deepEqual(rb, [{ id: 1 }])
})

test('makeQueryFn surfaces run rejections to every consumer', async () => {
  let runs = 0
  const q = makeQueryFn(async () => {
    runs += 1
    throw new Error('boom')
  })

  const a = q`select 1`
  await assert.rejects(async () => {
    await a
  }, /boom/)
  await assert.rejects(async () => {
    await a
  }, /boom/)
  assert.equal(runs, 1)
})

test('makeQueryFn carries the fragment strings/values for engine reuse', () => {
  const q = makeQueryFn(async () => [])
  const frag = q`select id from t where id = ${42}`
  assert.ok(Array.isArray((frag as any).strings))
  assert.ok(Array.isArray((frag as any).values))
})
