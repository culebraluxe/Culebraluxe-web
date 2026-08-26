// ---------------------------------------------------------------------------
// DB-HARDEN-01 — schema-drift preflight (082-style missing column detection).
//
// Proves the schema capability assertion tool reports a missing column as a
// `missing` entry (drift), and is positive when the column exists — WITHOUT a
// real database (fault-injectable executor).
// ---------------------------------------------------------------------------

import { test, afterEach } from 'node:test'
import assert from 'node:assert/strict'

import { assertSchemaCapabilities } from '../../db/schema-preflight'
import { setDatabaseTestExecutor } from '../../db/client'
import type { QueryExecutor } from '../../db/query-executor'

afterEach(() => setDatabaseTestExecutor(null))

test('schema preflight detects a missing column (e.g. property.is_published not promoted)', async () => {
  setDatabaseTestExecutor(
    (async () => [
      { table_name: 'property', column_name: 'id' },
      { table_name: 'property', column_name: 'name' },
      { table_name: 'property', column_name: 'status' },
    ]) as unknown as QueryExecutor,
  )
  const r = await assertSchemaCapabilities([
    { table: 'property', column: 'is_published' },
  ])
  assert.equal(r.ok, true)
  if (r.ok) assert.deepEqual(r.data.missing, ['property.is_published'])
})

test('schema preflight is positive when the column exists', async () => {
  setDatabaseTestExecutor(
    (async () => [
      { table_name: 'property', column_name: 'id' },
      { table_name: 'property', column_name: 'is_published' },
    ]) as unknown as QueryExecutor,
  )
  const r = await assertSchemaCapabilities([
    { table: 'property', column: 'is_published' },
  ])
  assert.equal(r.ok, true)
  if (r.ok) assert.deepEqual(r.data.missing, [])
})

test('schema preflight surfaces a gateway failure when the preflight query fails', async () => {
  setDatabaseTestExecutor(
    (async () => {
      throw Object.assign(new Error('connection refused'), { code: 'ECONNREFUSED' })
    }) as unknown as QueryExecutor,
  )
  const r = await assertSchemaCapabilities([
    { table: 'property', column: 'is_published' },
  ])
  assert.equal(r.ok, false)
  if (!r.ok) assert.equal(r.error.kind, 'DATABASE_UNAVAILABLE')
})
