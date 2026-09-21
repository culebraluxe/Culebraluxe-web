import assert from 'node:assert/strict'
import { dirname, resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import { createForgeReleaseOperations, type ForgeReleasePool } from '@/legacy/workflow_app/forge/release-operations'
import type { ParityReport } from '@/legacy/workflow_app/forge/schema-parity'

// ---------------------------------------------------------------------------
// ENG-FORGE-RELEASE-ORDER-01: a schema release cannot block itself on the
// difference it creates.
//
// The engine plans FORGE_VERIFY_DEV_MIGRATION BEFORE FORGE_MIGRATE_PROD, so a DEV
// verification runs while DEV legitimately holds the change PROD has not received
// yet. A cross-environment parity gate there fails the story on the drift the
// migration is about to remove. Parity must therefore run on the PROD
// verification, AFTER the apply, and still fail drift the migration did not explain.
// ---------------------------------------------------------------------------

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..')
const MIGRATION_FILE = '179_forge_kind_policy.sql'

// The parity checker is injected in every test, so these placeholders are never
// dialed; they only satisfy the gate's own "both URLs configured" precondition.
process.env.DATABASE_URL_DEV ??= 'postgres://dev.placeholder.invalid/parity'
process.env.DATABASE_URL_PROD ??= 'postgres://prod.placeholder.invalid/parity'

function parityReport(overrides: Partial<ParityReport> = {}): ParityReport {
  return {
    tablesOnlyDev: [],
    tablesOnlyProd: [],
    columnDrift: [],
    indexDrift: [],
    fkDrift: [],
    checkDrift: [],
    clean: true,
    ...overrides,
  }
}

/**
 * A fake pool that models the two durable rows verifyMigrations reads. The migration
 * SQL itself is opaque: the clean path accepts it and the failing path throws on it,
 * which is what a failed apply looks like to the operation.
 */
function fakeReleaseDb(options: { failMigrationSql?: boolean } = {}) {
  const state = { executionRows: 0, ledgerRows: 0 }
  const queries: string[] = []
  const pool: ForgeReleasePool = {
    async query(text: string) {
      const sql = text.replace(/\s+/g, ' ').trim()
      queries.push(sql)
      if (sql.includes('from forge_migration_execution')) {
        return { rows: [], rowCount: state.executionRows }
      }
      if (sql.includes('from schema_migration')) {
        return { rows: [], rowCount: state.ledgerRows }
      }
      if (sql.startsWith('insert into forge_migration_execution')) {
        state.executionRows = /,\s*true\s*,/.test(sql) ? 1 : 0
        return { rows: [], rowCount: 1 }
      }
      if (sql.startsWith('insert into schema_migration')) {
        state.ledgerRows = 1
        return { rows: [], rowCount: 1 }
      }
      if (sql === 'select 1') return { rows: [], rowCount: 1 }
      if (options.failMigrationSql) throw new Error('syntax error at or near "ALTER"')
      return { rows: [], rowCount: 1 }
    },
    async end() {},
  }
  return { pool, queries, state }
}

function input(target: 'dev' | 'prod') {
  return { storyId: 'STORY-1', target, migrationFiles: [MIGRATION_FILE], repoRoot }
}

test('ENG-FORGE-RELEASE-ORDER: the DEV verification never runs the parity gate', async () => {
  const db = fakeReleaseDb()
  let parityCalls = 0
  const operations = createForgeReleaseOperations({
    poolForTarget: () => db.pool,
    checkParity: async () => {
      parityCalls += 1
      return parityReport()
    },
  })

  assert.equal((await operations.applyMigrations({ ...input('dev'), commandId: 'cmd-dev' })).success, true)
  const verified = await operations.verifyMigrations(input('dev'))

  assert.equal(verified.success, true, 'a dev verify must not fail on the drift the migration is about to remove')
  assert.equal(parityCalls, 0, 'DEV legitimately leads PROD until the PROD migration runs')
})

test('ENG-FORGE-RELEASE-ORDER: the PROD verification runs parity after the applied migration and reads clean', async () => {
  const db = fakeReleaseDb()
  let parityCalls = 0
  let executionRowsAtParity = -1
  const operations = createForgeReleaseOperations({
    poolForTarget: () => db.pool,
    checkParity: async () => {
      parityCalls += 1
      executionRowsAtParity = db.state.executionRows
      return parityReport()
    },
  })

  const applied = await operations.applyMigrations({ ...input('prod'), commandId: 'cmd-prod' })
  assert.equal(applied.success, true)
  const verified = await operations.verifyMigrations(input('prod'))

  assert.equal(verified.success, true, 'the change the migration just applied must read as parity-clean')
  assert.equal(parityCalls, 1)
  assert.equal(executionRowsAtParity, 1, 'parity must run AFTER the migration is applied and recorded')
})

test('ENG-FORGE-RELEASE-ORDER: a failed PROD apply stops before parity', async () => {
  const db = fakeReleaseDb({ failMigrationSql: true })
  let parityCalls = 0
  const operations = createForgeReleaseOperations({
    poolForTarget: () => db.pool,
    checkParity: async () => {
      parityCalls += 1
      return parityReport()
    },
  })

  const applied = await operations.applyMigrations({ ...input('prod'), commandId: 'cmd-prod' })
  assert.equal(applied.success, false)
  const verified = await operations.verifyMigrations(input('prod'))

  assert.equal(verified.success, false)
  assert.match(verified.detail, /no successful checksum-matched execution/)
  assert.equal(parityCalls, 0, 'a failed apply must be a migration failure, never a parity failure')
})

test('ENG-FORGE-RELEASE-ORDER: drift the migration did not explain still fails the PROD gate and names it', async () => {
  const db = fakeReleaseDb()
  const operations = createForgeReleaseOperations({
    poolForTarget: () => db.pool,
    checkParity: async () => parityReport({ tablesOnlyDev: ['media_unrelated'], clean: false }),
  })

  await operations.applyMigrations({ ...input('prod'), commandId: 'cmd-prod' })
  const verified = await operations.verifyMigrations(input('prod'))

  assert.equal(verified.success, false)
  assert.match(verified.detail, /table only in DEV: media_unrelated/)
})
