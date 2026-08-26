// ---------------------------------------------------------------------------
// DB-HARDEN-01C — public runtime containment: zero throw-through on DB failure.
//
// Every public read service must return a typed Result failure (never throw a
// DbFailureError / raw driver error) so a public page can degrade locally.
// These prove the 42703 schema-drift condition across the public property +
// marketing read services, and the parallel-orchestration shape the homepage
// uses (one optional read fails, the other stays usable).
// ---------------------------------------------------------------------------

import { test, afterEach } from 'node:test'
import assert from 'node:assert/strict'

import { setDatabaseTestExecutor } from '../../db/client'
import {
  getProperties,
  getFilteredProperties,
  getPropertyBySlug,
} from '../../db/properties'
import { getMarketingContent } from '../../db/marketing-content'
import type { QueryExecutor } from '../../db/query-executor'

afterEach(() => setDatabaseTestExecutor(null))

function fault(code: string): QueryExecutor {
  return async () => {
    const e = new Error('fault injection')
    ;(e as { code?: string }).code = code
    throw e
  }
}

test('A: homepage property read fails -> Result failure, no throw', async () => {
  setDatabaseTestExecutor(fault('42703'))
  const r = await getProperties({ publicOnly: true })
  assert.equal(r.ok, false)
  if (!r.ok) assert.equal(r.error.kind, 'SCHEMA_MISMATCH')
})

test('B: buyers filtered read fails -> Result failure, not fake-empty-success', async () => {
  setDatabaseTestExecutor(fault('42703'))
  const r = await getFilteredProperties({})
  assert.equal(r.ok, false)
})

test('C/D: property detail read failure is distinct from genuine zero-row 404', async () => {
  setDatabaseTestExecutor(fault('42703'))
  const fail = await getPropertyBySlug('x')
  assert.equal(fail.ok, false, 'DB failure is a typed failure')

  setDatabaseTestExecutor(async () => [])
  const empty = await getPropertyBySlug('x')
  assert.equal(empty.ok, true, 'zero rows is a SUCCESS')
  if (empty.ok) assert.equal(empty.data, null, 'zero rows -> null (genuine 404)')
})

test('E: marketing content read fails -> Result failure, no throw', async () => {
  setDatabaseTestExecutor(fault('42703'))
  const r = await getMarketingContent()
  assert.equal(r.ok, false)
})

test('F: parallel — one read fails, one succeeds; the successful one stays usable', async () => {
  let calls = 0
  setDatabaseTestExecutor(async () => {
    calls++
    if (calls === 1) {
      const e = new Error('fault injection')
      ;(e as { code?: string }).code = '42703'
      throw e
    }
    return [] // marketing content: zero rows -> success
  })
  const [propsResult, contentResult] = await Promise.all([
    getProperties({ publicOnly: true }),
    getMarketingContent(),
  ])
  assert.equal(propsResult.ok, false, 'property read degraded')
  assert.equal(contentResult.ok, true, 'marketing read stayed usable')
})
