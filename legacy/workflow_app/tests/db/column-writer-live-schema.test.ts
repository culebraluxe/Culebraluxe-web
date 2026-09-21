import { test } from 'node:test'
import assert from 'node:assert/strict'
import { AUDITED_TABLES, classify, loadLiveColumns } from '@/scripts/column-writer-audit'

/**
 * THE LIVE HALF OF ENG-FORGE-COLUMN-WRITER-01, and the only part of that fence that needs a database.
 *
 * It moved to `legacy/workflow_app/tests/db/` on 2026-09-18 so the DB-free `pnpm test:app` glob could finally
 * mean what its documentation claimed — but the CLASSIFIER tests stayed at the top level on purpose:
 * they need no database, and moving the whole file would have quietly dropped four running tests out of
 * CI. Only the live-schema check belongs here.
 *
 * A schema that cannot be reached FAILS rather than skips: a silent pass is how a column that describes a
 * machine nobody wrote stays a lie. That is why this file is on demand (`pnpm test:app:db`) rather than
 * self-skipping, and why it must never be added to a no-database gate.
 */
test('the LIVE schema of the three tables is fully classified', async () => {
  let columns: Record<string, string[]>
  try {
    columns = await loadLiveColumns()
  } catch (error) {
    assert.fail(
      `could not read the live schema (information_schema.columns): ${error instanceof Error ? error.message : String(error)}`,
    )
  }
  for (const table of AUDITED_TABLES) {
    assert.ok(
      (columns[table] ?? []).length > 0,
      `no live columns read for ${table} — the audit would be silently empty`,
    )
  }
  const result = classify(columns)
  assert.deepEqual(
    result.unclassified,
    [],
    'a live column is unclassified — add its writer or its reason to DECLARED_COLUMNS in scripts/column-writer-audit.ts, then regenerate the manifest',
  )
  assert.deepEqual(
    result.stale,
    [],
    'a declared column no longer exists in the schema — remove it from DECLARED_COLUMNS',
  )
  assert.deepEqual(result.missingWriters, [], 'a declared writer path does not exist on disk')
})
