import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import {
  MIGRATION_ATTEMPT_DETAIL,
  createForgeReleaseOperations,
  isDataMigration,
  isIdempotentDataMigration,
  type ForgeReleasePool,
} from '../forge/release-operations'

// ---------------------------------------------------------------------------
// ENG-FORGE-MIGRATION-REPLAY-01: a migration that already ran is not executed again.
//
// The release path used to run the SQL first and record the execution after, so a
// failure between the two repeated the SQL on retry. These tests drive the injected
// pool seam (no live database) and prove:
//   * an equal recorded checksum SKIPS execution and reports already-applied;
//   * a differing checksum REFUSES by name and executes nothing;
//   * a failed record write leaves an attempt row and the retry does not repeat the SQL;
//   * a non-idempotent data migration refuses to replay, while DDL and idempotent data
//     migrations may retry.
// ---------------------------------------------------------------------------

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..')

const DDL_FILE = '179_forge_kind_policy.sql'
const NON_IDEMPOTENT_DATA_FILE = '163_cleanup_applemail_integer_message_identities.sql'
const IDEMPOTENT_DATA_FILE = '097_source_person_link.sql'

async function sha256Of(file: string): Promise<string> {
  const sql = await readFile(resolve(repoRoot, 'db', 'migrations', file), 'utf8')
  return createHash('sha256').update(sql, 'utf8').digest('hex')
}

type AttemptRow = { success: boolean; content_sha256: string; detail: string | null }

/** A fake pool that models the two ledger rows applyMigrations reads, and counts migration SQL runs. */
function fakeReleaseDb(
  options: {
    attemptRow?: AttemptRow | null
    ledgerRow?: { checksum: string } | null
    failSuccessRecord?: boolean
  } = {},
) {
  const state: { attemptRow: AttemptRow | null; ledgerRow: { checksum: string } | null } = {
    attemptRow: options.attemptRow ?? null,
    ledgerRow: options.ledgerRow ?? null,
  }
  const queries: string[] = []
  const pool: ForgeReleasePool = {
    async query(text: string, params?: unknown[]) {
      const sql = text.replace(/\s+/g, ' ').trim()
      queries.push(sql)
      if (sql.includes('from forge_migration_execution')) {
        return { rows: state.attemptRow ? [state.attemptRow] : [], rowCount: state.attemptRow ? 1 : 0 }
      }
      if (sql.includes('from schema_migration')) {
        return { rows: state.ledgerRow ? [state.ledgerRow] : [], rowCount: state.ledgerRow ? 1 : 0 }
      }
      if (sql.startsWith('insert into forge_migration_execution')) {
        const success = /,\s*true\s*,/.test(sql)
        if (success && options.failSuccessRecord) throw new Error('record write failed: connection reset')
        state.attemptRow = {
          success,
          content_sha256: String(params?.[4] ?? ''),
          detail: params?.[5] == null ? null : String(params[5]),
        }
        return { rows: [], rowCount: 1 }
      }
      if (sql.startsWith('insert into schema_migration')) {
        state.ledgerRow = { checksum: String(params?.[1] ?? '') }
        return { rows: [], rowCount: 1 }
      }
      if (sql === 'select 1') return { rows: [], rowCount: 1 }
      // Anything else is the migration file's own SQL.
      return { rows: [], rowCount: 1 }
    },
    async end() {},
  }
  const isLedgerStatement = (sql: string) =>
    sql.includes('from forge_migration_execution') ||
    sql.includes('from schema_migration') ||
    sql.startsWith('insert into forge_migration_execution') ||
    sql.startsWith('insert into schema_migration') ||
    sql === 'select 1'
  return {
    pool,
    state,
    migrationRuns: () => queries.filter((sql) => !isLedgerStatement(sql)).length,
  }
}

function operationsWith(pool: ForgeReleasePool) {
  return createForgeReleaseOperations({ poolForTarget: () => pool })
}

function input(target: 'dev' | 'prod', file: string, commandId: string) {
  return { commandId, storyId: 'STORY-1', target, migrationFiles: [file], repoRoot }
}

test('classifier: only statement-leading data keywords count', () => {
  assert.equal(isDataMigration('begin; create table if not exists t (id int); commit;'), false)
  assert.equal(isDataMigration('alter table t add column x int;'), false)
  assert.equal(isDataMigration('create trigger trg after update on t for each row execute procedure f();'), false)
  assert.equal(isDataMigration('insert into t (a) values (1);'), true)
  assert.equal(isDataMigration('update t set a = 1 where b = 2;'), true)
  assert.equal(isDataMigration('delete from t where a = 1;'), true)
  assert.equal(isIdempotentDataMigration('insert into t (a) values (1) on conflict (a) do nothing;'), true)
  assert.equal(isIdempotentDataMigration('insert into t select 1 where not exists (select 1 from t);'), true)
  assert.equal(isIdempotentDataMigration('delete from t where a = 1;'), false)
})

test('replay guard: an equal recorded checksum skips execution and reports already-applied', async () => {
  const sha = await sha256Of(DDL_FILE)
  const db = fakeReleaseDb({
    attemptRow: { success: true, content_sha256: sha, detail: 'migration SQL executed without error' },
  })

  const result = await operationsWith(db.pool).applyMigrations(input('prod', DDL_FILE, 'cmd-2'))

  assert.equal(result.success, true)
  assert.match(result.detail, /already applied/)
  assert.match(result.detail, new RegExp(DDL_FILE))
  assert.equal(db.migrationRuns(), 0, 'an applied migration must not execute again')
})

test('replay guard: a differing checksum refuses by name and executes nothing', async () => {
  const db = fakeReleaseDb({
    attemptRow: { success: true, content_sha256: 'deadbeef', detail: 'migration SQL executed without error' },
  })

  const result = await operationsWith(db.pool).applyMigrations(input('prod', DDL_FILE, 'cmd-2'))

  assert.equal(result.success, false)
  assert.match(result.detail, new RegExp(DDL_FILE))
  assert.match(result.detail, /different checksum/)
  assert.equal(db.migrationRuns(), 0, 'a changed migration must never be re-run over an applied one')
})

test('replay guard: a failed record write leaves an attempt row and the retry does not repeat the SQL', async () => {
  const sha = await sha256Of(DDL_FILE)
  const db = fakeReleaseDb({ failSuccessRecord: true })
  const operations = operationsWith(db.pool)

  const first = await operations.applyMigrations(input('prod', DDL_FILE, 'cmd-first'))
  assert.equal(first.success, false)
  assert.equal(db.migrationRuns(), 1, 'the first attempt executes the SQL once')
  assert.equal(db.state.attemptRow?.success, false)
  assert.equal(db.state.attemptRow?.content_sha256, sha)
  assert.equal(db.state.attemptRow?.detail, MIGRATION_ATTEMPT_DETAIL)

  const retry = await operations.applyMigrations(input('prod', DDL_FILE, 'cmd-retry'))
  assert.equal(retry.success, false)
  assert.match(retry.detail, /outcome was never recorded/)
  assert.equal(db.migrationRuns(), 1, 'a recording failure must not cause the SQL to run twice')
})

test('first run still writes both ledger records', async () => {
  const sha = await sha256Of(DDL_FILE)
  const db = fakeReleaseDb()

  const result = await operationsWith(db.pool).applyMigrations(input('prod', DDL_FILE, 'cmd-first'))

  assert.equal(result.success, true)
  assert.equal(db.migrationRuns(), 1)
  assert.equal(db.state.attemptRow?.success, true)
  assert.equal(db.state.attemptRow?.content_sha256, sha)
  assert.equal(db.state.ledgerRow?.checksum, `sha256:${sha}`)
})

test('data migration: a first run executes, a non-idempotent replay refuses by name', async () => {
  const sha = await sha256Of(NON_IDEMPOTENT_DATA_FILE)

  const first = fakeReleaseDb()
  const firstResult = await operationsWith(first.pool).applyMigrations(
    input('prod', NON_IDEMPOTENT_DATA_FILE, 'cmd-first'),
  )
  assert.equal(firstResult.success, true)
  assert.equal(first.migrationRuns(), 1, 'a data migration executes on its first run')

  const replay = fakeReleaseDb({
    attemptRow: { success: false, content_sha256: sha, detail: 'failed: syntax error' },
  })
  const replayResult = await operationsWith(replay.pool).applyMigrations(
    input('prod', NON_IDEMPOTENT_DATA_FILE, 'cmd-retry'),
  )
  assert.equal(replayResult.success, false)
  assert.match(replayResult.detail, new RegExp(NON_IDEMPOTENT_DATA_FILE))
  assert.match(replayResult.detail, /non-idempotent data migration/)
  assert.equal(replay.migrationRuns(), 0, 'a non-idempotent data migration must not replay')
})

test('data migration: an idempotent guard and DDL both allow a retry', async () => {
  const idempotentSha = await sha256Of(IDEMPOTENT_DATA_FILE)
  const idempotent = fakeReleaseDb({
    attemptRow: { success: false, content_sha256: idempotentSha, detail: 'failed: syntax error' },
  })
  const idempotentResult = await operationsWith(idempotent.pool).applyMigrations(
    input('prod', IDEMPOTENT_DATA_FILE, 'cmd-retry'),
  )
  assert.equal(idempotentResult.success, true)
  assert.equal(idempotent.migrationRuns(), 1, 'an idempotent data migration may retry')

  const ddlSha = await sha256Of(DDL_FILE)
  const ddl = fakeReleaseDb({
    attemptRow: { success: false, content_sha256: ddlSha, detail: 'failed: syntax error' },
  })
  const ddlResult = await operationsWith(ddl.pool).applyMigrations(input('prod', DDL_FILE, 'cmd-retry'))
  assert.equal(ddlResult.success, true)
  assert.equal(ddl.migrationRuns(), 1, 'a DDL migration may retry')
})

test('canonical ledger: a migration recorded in schema_migration is skipped', async () => {
  const sha = await sha256Of(NON_IDEMPOTENT_DATA_FILE)
  const db = fakeReleaseDb({ ledgerRow: { checksum: `sha256:${sha}` } })

  const result = await operationsWith(db.pool).applyMigrations(
    input('prod', NON_IDEMPOTENT_DATA_FILE, 'cmd-2'),
  )

  assert.equal(result.success, true)
  assert.match(result.detail, /already applied/)
  assert.match(result.detail, new RegExp(NON_IDEMPOTENT_DATA_FILE))
  assert.equal(db.migrationRuns(), 0, 'a migration recorded by any writer must not be replayed')
})
