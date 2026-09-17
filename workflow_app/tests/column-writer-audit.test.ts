// ---------------------------------------------------------------------------
// ENG-FORGE-COLUMN-WRITER-01 — the audit cannot go quiet.
//
// The generated audit (scripts/column-writer-audit.ts → docs/agent/manifest/
// COLUMN-WRITER-AUDIT.md) is only worth its bytes if a NEW column fails it. This
// test proves the mechanism on a fixture (an undeclared column is UNCLASSIFIED)
// and then runs the same classifier against the LIVE schema: a column added to
// storyboard_story, storyboard_story_run or forge_workflow_evidence by any future
// migration fails here until the audit names its writer or its reason.
//
// A schema that cannot be reached FAILS rather than skips: a silent pass is how a
// column that describes a machine nobody wrote stays a lie.
// ---------------------------------------------------------------------------

import { after, test } from 'node:test'
import assert from 'node:assert/strict'
import {
  AUDITED_TABLES,
  DECLARED_COLUMNS,
  classify,
  loadLiveColumns,
  renderManifest,
} from '../../scripts/column-writer-audit'
import { forgeDb } from '../../db/forge-db'

after(async () => {
  await forgeDb.end()
})

/** The declared map expressed as a schema, so classify() can validate the map itself. */
function declaredAsColumns(): Record<string, string[]> {
  const columns: Record<string, string[]> = {}
  for (const table of Object.keys(DECLARED_COLUMNS)) {
    columns[table] = Object.keys(DECLARED_COLUMNS[table])
  }
  return columns
}

test('an undeclared column is UNCLASSIFIED (the failure the audit exists to raise)', () => {
  const result = classify({
    storyboard_story: ['id', 'a_column_no_migration_ever_declared'],
  })
  assert.deepEqual(result.unclassified, [
    { table: 'storyboard_story', column: 'a_column_no_migration_ever_declared' },
  ])
  const row = result.rows.find((r) => r.column === 'a_column_no_migration_ever_declared')
  assert.equal(row?.classification, 'UNCLASSIFIED')
})

test('every declared column carries a writer path or a non-empty reason', () => {
  for (const [table, columns] of Object.entries(DECLARED_COLUMNS)) {
    for (const [column, classification] of Object.entries(columns)) {
      if (classification.kind === 'WRITTEN') {
        assert.ok(
          classification.writers.length > 0,
          `${table}.${column} is WRITTEN but names no writer`,
        )
      } else {
        assert.ok(
          classification.reason.trim().length > 0,
          `${table}.${column} is ${classification.kind} with no reason — an unexplained keep/drop is a lie`,
        )
      }
    }
  }
})

test('every declared writer path exists on disk (the map cites no dead path)', () => {
  const result = classify(declaredAsColumns())
  assert.deepEqual(result.missingWriters, [])
})

test('the rendered manifest is byte-stable and free of UNCLASSIFIED', () => {
  const result = classify(declaredAsColumns())
  assert.deepEqual(result.unclassified, [])
  assert.deepEqual(result.stale, [])
  const first = renderManifest(result)
  const second = renderManifest(result)
  assert.equal(first, second, 'regeneration must be idempotent')
  assert.ok(!first.includes('UNCLASSIFIED'))
  for (const table of AUDITED_TABLES) {
    assert.ok(first.includes(`## ${table} (`), `manifest is missing the ${table} section`)
  }
})

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
