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

import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  AUDITED_TABLES,
  DECLARED_COLUMNS,
  classify,
  renderManifest,
} from '@/scripts/column-writer-audit'

// NO DATABASE HANDLE HERE, ON PURPOSE. This file's `after(() => forgeDb.end())` cleanup used to live at
// the top level, which made a DB-free test file require APP_ENV at load: with no control plane declared,
// the hook itself threw `ExecutionTargetError` and turned four classifier tests red in a no-database gate.
// The handle moved with the live-schema test, which is the only thing here that ever opened one.

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

// THE LIVE-SCHEMA HALF MOVED: `legacy/workflow_app/tests/db/column-writer-live-schema.test.ts`.
//
// It needs the control plane (the classifier tests below need nothing), so it lives in the db/ directory
// that the DB-free `pnpm test:app` glob deliberately excludes — while these classifier tests stay here,
// because moving the whole file would have dropped four running tests out of CI to satisfy a convention.
