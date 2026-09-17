// ---------------------------------------------------------------------------
// ENG-FORGE-MIGRATION-APPLIED-01 — both directions, with an injected ledger.
//
// The ledger is a parameter, so this proof reads NO database and NO filesystem: a
// fixture change set adding a migration with an empty ledger is refused BY NAME, and
// the same change set with the ledger row is allowed. A change set with no migration
// is unaffected — no false positives in either direction.
// ---------------------------------------------------------------------------

import { describe, it } from 'node:test'
import assert from 'node:assert/strict'

import {
  assessMigrationApplied,
  migrationAppliedRefusal,
  migrationFilesInChangeSet,
} from '../forge/migration-applied-guard'

const MIGRATION = 'db/migrations/179_forge_kind_policy.sql'

describe('migration-applied guard', () => {
  it('refuses a change set adding an unledgered migration, BY NAME', () => {
    const assessment = assessMigrationApplied({
      changedPaths: ['app/page.tsx', MIGRATION],
      ledgerFilenames: [],
    })
    assert.equal(assessment.ok, false)
    assert.deepEqual(assessment.unapplied, [MIGRATION])
    assert.match(migrationAppliedRefusal(assessment.unapplied), /179_forge_kind_policy\.sql/)
  })

  it('allows the same change set once the ledger carries the row', () => {
    const assessment = assessMigrationApplied({
      changedPaths: ['app/page.tsx', MIGRATION],
      ledgerFilenames: [MIGRATION],
    })
    assert.equal(assessment.ok, true)
    assert.deepEqual(assessment.unapplied, [])
  })

  it('a change set with no migration is unaffected (no false positive)', () => {
    const assessment = assessMigrationApplied({
      changedPaths: ['app/page.tsx', 'lib/thing.ts'],
      ledgerFilenames: [],
    })
    assert.equal(assessment.ok, true)
    assert.deepEqual(assessment.unapplied, [])
  })

  it('names EVERY unapplied file, not just the first', () => {
    const assessment = assessMigrationApplied({
      changedPaths: ['db/migrations/179_a.sql', 'db/migrations/180_b.sql'],
      ledgerFilenames: ['db/migrations/179_a.sql'],
    })
    assert.equal(assessment.ok, false)
    assert.deepEqual(assessment.unapplied, ['db/migrations/180_b.sql'])
    assert.match(migrationAppliedRefusal(assessment.unapplied), /180_b\.sql/)
  })

  it('only db/migrations/*.sql paths count as migrations', () => {
    assert.deepEqual(
      migrationFilesInChangeSet([
        'db/migrations/179_a.sql',
        'db/migrations/179_a.sql',
        'db/seeds/001_seed.sql',
        'db/migrations/README.md',
        'app/page.tsx',
      ]),
      ['db/migrations/179_a.sql'],
    )
  })
})
