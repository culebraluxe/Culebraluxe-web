import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { basename, relative, resolve, sep } from 'node:path'
import { forgeDb, forgeDbTargetForUrl } from '../../db/forge-db'
import { checkSchemaParity } from './schema-parity'

export type ForgeReleaseTarget = 'dev' | 'prod'
export type ForgeOperationResult = { success: boolean; detail: string }

export interface ForgeReleaseOperations {
  applyMigrations(input: {
    commandId: string
    storyId: string
    target: ForgeReleaseTarget
    migrationFiles: string[]
    repoRoot: string
  }): Promise<ForgeOperationResult>
  verifyMigrations(input: {
    storyId: string
    target: ForgeReleaseTarget
    migrationFiles: string[]
    repoRoot: string
  }): Promise<ForgeOperationResult>
  refreshDerived(input: {
    commandId: string
    storyId: string
    target: ForgeReleaseTarget
    models: string[]
  }): Promise<ForgeOperationResult>
  verifyDerived(input: {
    storyId: string
    target: ForgeReleaseTarget
    models: string[]
  }): Promise<ForgeOperationResult>
}

function databaseUrl(target: ForgeReleaseTarget): string {
  const url = target === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  if (!url) throw new Error(`DATABASE_URL_${target.toUpperCase()} is not configured`)
  return url
}

function migrationPath(repoRoot: string, file: string): string {
  const migrationsRoot = resolve(repoRoot, 'db', 'migrations')
  const candidate = resolve(migrationsRoot, file)
  const pathFromRoot = relative(migrationsRoot, candidate)
  if (
    pathFromRoot.startsWith(`..${sep}`) ||
    pathFromRoot === '..' ||
    !/^\d{3}_[a-z0-9_-]+\.sql$/i.test(basename(candidate))
  ) {
    throw new Error(`unsafe Forge migration path: ${file}`)
  }
  return candidate
}

function quotedModel(model: string): string {
  if (!/^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)?$/i.test(model)) {
    throw new Error(`unsafe derived model identifier: ${model}`)
  }
  return model
    .split('.')
    .map((part) => `"${part}"`)
    .join('.')
}

async function migrationContents(repoRoot: string, files: string[]) {
  if (files.length === 0) throw new Error('migrationRequired=true but migrationFiles is empty')
  return Promise.all(
    files.map(async (file) => {
      const path = migrationPath(repoRoot, file)
      const sql = await readFile(path, 'utf8')
      return {
        file: basename(path),
        inputFile: file,
        sql,
        sha256: createHash('sha256').update(sql, 'utf8').digest('hex'),
      }
    }),
  )
}

/** The narrow pool surface these operations use. Production is ForgeDB's handle. */
export interface ForgeReleasePool {
  query(text: string, params?: unknown[]): Promise<{ rows: unknown[]; rowCount: number | null }>
  end(): Promise<void>
}

/**
 * Optional seams. Both default to the production implementations, so a caller that
 * passes nothing keeps today's behavior exactly. They exist so the apply-then-verify
 * ordering is drivable in a test without a live Neon connection.
 */
export interface ForgeReleaseOperationsDeps {
  poolForTarget?: (target: ForgeReleaseTarget) => ForgeReleasePool
  checkParity?: typeof checkSchemaParity
}

// ---------------------------------------------------------------------------
// ENG-FORGE-MIGRATION-REPLAY-01 — a migration that already ran is not executed again.
//
// The apply path used to run `migration.sql` FIRST and record the execution after, so a
// failure between the two repeated the SQL on retry. Execution is now gated by the ledger:
// an equal checksum on a recorded success SKIPS, a differing checksum REFUSES by name, and
// an attempt row is written BEFORE the SQL so a failed record write still leaves durable
// state the next attempt reads and refuses to repeat.
//
// A data migration (insert/update/delete outside DDL) is the systemic risk: a checksum
// guard cannot make a migration safe that was never safe to repeat. Such a migration is
// replayed only when it carries a structural idempotency guard (`on conflict` /
// `where not exists`); otherwise a retry refuses by name.
// ---------------------------------------------------------------------------

/** Detail written before a migration's SQL runs; its presence means the outcome was never recorded. */
export const MIGRATION_ATTEMPT_DETAIL = 'attempting: SQL outcome not yet recorded'
/** Prefix marking a recorded SQL failure (a known outcome, retryable for DDL). */
export const MIGRATION_FAILURE_PREFIX = 'failed: '

/** Strip SQL line and block comments so classification reads statements, not prose. */
function withoutSqlComments(sql: string): string {
  return sql.replace(/--[^\n]*/g, ' ').replace(/\/\*[\s\S]*?\*\//g, ' ')
}

/**
 * True when any top-level statement mutates data (insert/update/delete) rather than only
 * schema. Statement-leading keywords only, so `on conflict ... do update` inside an insert
 * and `create trigger ... update` are not misread as data statements.
 */
export function isDataMigration(sql: string): boolean {
  return withoutSqlComments(sql)
    .split(';')
    .map((statement) => statement.trim().replace(/\s+/g, ' ').toLowerCase())
    .some(
      (statement) =>
        statement.startsWith('insert into') ||
        statement.startsWith('update ') ||
        statement === 'update' ||
        statement.startsWith('delete from'),
    )
}

/** True when a data migration carries a structural guard that makes a repeat a no-op. */
export function isIdempotentDataMigration(sql: string): boolean {
  const body = withoutSqlComments(sql).toLowerCase()
  return body.includes('on conflict') || body.includes('where not exists')
}

interface RecordedMigration {
  success: boolean
  content_sha256: string
  detail: string | null
}

/**
 * The latest recorded execution for a (target, migration_file), ACROSS command ids: a retry
 * carries a new commandId, so a read scoped to this command would miss the attempt that must
 * stop the replay.
 */
async function readLatestMigrationExecution(
  pool: ForgeReleasePool,
  target: ForgeReleaseTarget,
  file: string,
): Promise<RecordedMigration | null> {
  const result = await pool.query(
    `select success, content_sha256, detail
       from forge_migration_execution
      where target = $1 and migration_file = $2
      order by executed_at desc
      limit 1`,
    [target, file],
  )
  return (result.rows[0] as RecordedMigration | undefined) ?? null
}

/**
 * The canonical schema_migration checksum for this file, whichever filename form it was
 * recorded under. The canonical ledger is written by every migration writer, so a data
 * migration recorded there is never replayed by this path.
 */
async function readCanonicalMigrationChecksum(
  pool: ForgeReleasePool,
  target: ForgeReleaseTarget,
  inputFile: string,
  file: string,
): Promise<string | null> {
  const result = await pool.query(
    `select checksum from schema_migration
      where target = $1 and (filename = $2 or filename = $3)
      order by applied_at desc
      limit 1`,
    [target, inputFile, file],
  )
  const row = result.rows[0] as { checksum?: unknown } | undefined
  return row?.checksum == null ? null : String(row.checksum)
}

type ReplayDecision =
  | { kind: 'execute' }
  | { kind: 'skip'; detail: string }
  | { kind: 'refuse'; detail: string }

/** The pre-execution decision: skip an applied file, refuse a changed or unrecorded one, else execute. */
async function assessMigrationReplay(
  pool: ForgeReleasePool,
  input: {
    target: ForgeReleaseTarget
    file: string
    inputFile: string
    sha256: string
    sql: string
  },
): Promise<ReplayDecision> {
  const canonical = await readCanonicalMigrationChecksum(pool, input.target, input.inputFile, input.file)
  if (canonical != null) {
    if (canonical !== `sha256:${input.sha256}`) {
      return {
        kind: 'refuse',
        detail:
          `${input.file} is recorded in the schema_migration ledger for ${input.target} with a different ` +
          `checksum (${canonical}); refusing to re-run a changed migration`,
      }
    }
    return {
      kind: 'skip',
      detail: `${input.file} is already recorded in the schema_migration ledger for ${input.target}; skipping execution`,
    }
  }

  const recorded = await readLatestMigrationExecution(pool, input.target, input.file)
  if (recorded == null) return { kind: 'execute' }

  if (recorded.content_sha256 !== input.sha256) {
    return {
      kind: 'refuse',
      detail:
        `${input.file} was already applied on ${input.target} with a different checksum ` +
        `(recorded ${recorded.content_sha256}, now ${input.sha256}); refusing to re-run a changed migration`,
    }
  }
  if (recorded.success) {
    return {
      kind: 'skip',
      detail: `${input.file} is already applied on ${input.target} (checksum matched); skipping execution`,
    }
  }
  if (recorded.detail === MIGRATION_ATTEMPT_DETAIL) {
    return {
      kind: 'refuse',
      detail:
        `${input.file} was attempted on ${input.target} but its outcome was never recorded ` +
        `(recording failure); refusing to re-run it without operator confirmation`,
    }
  }
  // A recorded SQL failure is a known outcome. DDL is idempotent by repo convention; a data
  // migration must carry its own idempotency guard or the retry is refused.
  if (isDataMigration(input.sql) && !isIdempotentDataMigration(input.sql)) {
    return {
      kind: 'refuse',
      detail:
        `${input.file} is a non-idempotent data migration that already failed on ${input.target}; ` +
        `refusing to re-run it because a repeat could change data twice`,
    }
  }
  return { kind: 'execute' }
}

export function createForgeReleaseOperations(deps: ForgeReleaseOperationsDeps = {}): ForgeReleaseOperations {
  const poolForTarget =
    deps.poolForTarget ??
    ((target: ForgeReleaseTarget): ForgeReleasePool => {
      const handle = forgeDb.forTarget(forgeDbTargetForUrl(databaseUrl(target)))
      return {
        query: (text, params) => handle.query(text, params),
        end: () => handle.end(),
      }
    })
  const checkParity = deps.checkParity ?? checkSchemaParity
  return {
    async applyMigrations(input) {
      const migrations = await migrationContents(input.repoRoot, input.migrationFiles)
      const pool = poolForTarget(input.target)
      const completed: string[] = []
      const skipped: string[] = []
      try {
        for (const migration of migrations) {
          let decision: ReplayDecision
          try {
            decision = await assessMigrationReplay(pool, {
              target: input.target,
              file: migration.file,
              inputFile: migration.inputFile,
              sha256: migration.sha256,
              sql: migration.sql,
            })
          } catch (error) {
            const detail = String((error as Error)?.message ?? error)
            return {
              success: false,
              detail: `${migration.file} could not be checked against the migration ledger on ${input.target}: ${detail}`,
            }
          }
          if (decision.kind === 'refuse') return { success: false, detail: decision.detail }
          if (decision.kind === 'skip') {
            skipped.push(migration.file)
            continue
          }

          // Record the attempt BEFORE the SQL. A failure here aborts without executing; a
          // failure of the success record AFTER execution leaves this row behind, so the next
          // attempt reads it and refuses instead of repeating the SQL.
          try {
            await pool.query(
              `insert into forge_migration_execution
                 (command_id, story_id, target, migration_file, content_sha256, success, detail)
               values ($1, $2, $3, $4, $5, false, $6)
               on conflict (command_id, migration_file) do update
                 set content_sha256 = excluded.content_sha256, success = false, detail = excluded.detail, executed_at = now()`,
              [
                input.commandId,
                input.storyId,
                input.target,
                migration.file,
                migration.sha256,
                MIGRATION_ATTEMPT_DETAIL,
              ],
            )
          } catch (error) {
            const detail = String((error as Error)?.message ?? error)
            return {
              success: false,
              detail: `${migration.file} could not be recorded as an attempt on ${input.target}; refusing to execute it: ${detail}`,
            }
          }

          try {
            await pool.query(migration.sql)
          } catch (error) {
            const detail = String((error as Error)?.message ?? error)
            await pool
              .query(
                `insert into forge_migration_execution
                   (command_id, story_id, target, migration_file, content_sha256, success, detail)
                 values ($1, $2, $3, $4, $5, false, $6)
                 on conflict (command_id, migration_file) do update
                   set success = false, detail = excluded.detail, executed_at = now()`,
                [
                  input.commandId,
                  input.storyId,
                  input.target,
                  migration.file,
                  migration.sha256,
                  `${MIGRATION_FAILURE_PREFIX}${detail}`,
                ],
              )
              .catch(() => undefined)
            return {
              success: false,
              detail: `${migration.file} failed after [${completed.join(', ')}]: ${detail}`,
            }
          }

          try {
            await pool.query(
              `insert into forge_migration_execution
                 (command_id, story_id, target, migration_file, content_sha256, success, detail)
               values ($1, $2, $3, $4, $5, true, $6)
               on conflict (command_id, migration_file) do update
                 set success = true, detail = excluded.detail, executed_at = now()`,
              [
                input.commandId,
                input.storyId,
                input.target,
                migration.file,
                migration.sha256,
                'migration SQL executed without error',
              ],
            )
          } catch (error) {
            const detail = String((error as Error)?.message ?? error)
            return {
              success: false,
              detail:
                `${migration.file} was executed on ${input.target} but its execution record could not be ` +
                `updated (the attempt row remains; a retry will refuse rather than repeat): ${detail}`,
            }
          }
          completed.push(migration.file)

          // Durable ledger (migration 144): the canonical record of what was applied where,
          // independent of this story's execution history.
          try {
            await pool.query(
              `insert into schema_migration (filename, checksum, target, note)
               values ($1, $2, $3, $4)
               on conflict (filename, target)
               do update set checksum = excluded.checksum, applied_at = now(), note = excluded.note`,
              [migration.inputFile, `sha256:${migration.sha256}`, input.target, `forge story ${input.storyId}`],
            )
          } catch (ledgerError) {
            const detail = String((ledgerError as Error)?.message ?? ledgerError)
            return {
              success: false,
              detail: `${migration.file} applied but could not be recorded in the schema_migration ledger (apply db/migrations/144_schema_migration_ledger.sql): ${detail}`,
            }
          }
        }
        const appliedDetail = completed.length > 0 ? `applied ${completed.join(', ')} to ${input.target}` : ''
        const skippedDetail = skipped.length > 0 ? `skipped ${skipped.join(', ')} on ${input.target} (already applied)` : ''
        return {
          success: true,
          detail: [appliedDetail, skippedDetail].filter(Boolean).join('; ') || `no migrations to apply on ${input.target}`,
        }
      } finally {
        await pool.end()
      }
    },

    async verifyMigrations(input) {
      const migrations = await migrationContents(input.repoRoot, input.migrationFiles)
      const pool = poolForTarget(input.target)
      try {
        for (const migration of migrations) {
          const result = await pool.query(
            `select 1
             from forge_migration_execution
             where story_id = $1 and target = $2 and migration_file = $3
               and content_sha256 = $4 and success = true
             order by executed_at desc
             limit 1`,
            [input.storyId, input.target, migration.file, migration.sha256],
          )
          if (result.rowCount !== 1) {
            return {
              success: false,
              detail: `${migration.file} has no successful checksum-matched execution on ${input.target}`,
            }
          }

          // Durable ledger: a story-scoped execution row is not enough — the
          // canonical record must exist too, or "what is applied where" is
          // unanswerable again (the 2026-09-10 drift).
          const ledger = await pool.query(
            `select 1 from schema_migration where filename = $1 and target = $2 and checksum = $3`,
            [migration.inputFile, input.target, `sha256:${migration.sha256}`],
          )
          if (ledger.rowCount !== 1) {
            return {
              success: false,
              detail: `${migration.file} is not recorded in the schema_migration ledger for ${input.target} (apply db/migrations/144_schema_migration_ledger.sql, or re-run the migration command so it is recorded)`,
            }
          }
        }
        await pool.query('select 1')

        // DEV_OPS gate: no schema story reaches complete while DEV and PROD
        // structurally differ. A branch reset hides drift, so it is checked here
        // independently, on tables, columns, indexes and FKs.
        //
        // IT RUNS ONLY ON THE PROD VERIFICATION. The engine plans FORGE_VERIFY_DEV_MIGRATION
        // before FORGE_MIGRATE_PROD, so a DEV verify runs while DEV legitimately holds the
        // change PROD has not received yet — the drift the migration is about to remove. A
        // gate there fails the story on the difference it creates and blocks its own
        // promotion. The PROD verification runs after the apply, so this check reads clean
        // for the change just applied and still fails any drift the migration did not explain.
        if (input.target === 'prod') {
          const devUrl = process.env.DATABASE_URL_DEV
          const prodUrl = process.env.DATABASE_URL_PROD
          if (!devUrl || !prodUrl) {
            return {
              success: false,
              detail: 'DEV_OPS gate requires DATABASE_URL_DEV and DATABASE_URL_PROD to verify schema parity',
            }
          }
          const parity = await checkParity(devUrl, prodUrl)
          if (!parity.clean) {
            const lines = [
              ...parity.tablesOnlyDev.map((t) => `table only in DEV: ${t}`),
              ...parity.tablesOnlyProd.map((t) => `table only in PROD: ${t}`),
              ...parity.columnDrift,
              ...parity.indexDrift.map((d) => `index ${d}`),
              ...parity.fkDrift.map((d) => `fk ${d}`),
            ]
            return {
              success: false,
              detail: `schema parity FAILED between DEV and PROD (${lines.length} difference(s)): ${lines.slice(0, 8).join('; ')}${lines.length > 8 ? `; +${lines.length - 8} more` : ''}`,
            }
          }
          return {
            success: true,
            detail: `verified ${migrations.length} checksum-matched migration execution(s) on prod; ledger recorded; DEV/PROD schema parity OK`,
          }
        }
        return {
          success: true,
          detail: `verified ${migrations.length} checksum-matched migration execution(s) on dev; ledger recorded; DEV/PROD parity deferred until the PROD migration is applied`,
        }
      } finally {
        await pool.end()
      }
    },

    async refreshDerived(input) {
      if (input.models.length === 0) throw new Error('derivedRefreshRequired=true but derivedModels is empty')
      const pool = poolForTarget(input.target)
      try {
        for (const model of input.models) {
          try {
            await pool.query(`refresh materialized view concurrently ${quotedModel(model)}`)
            await pool.query(
              `insert into forge_derived_refresh_execution
                 (command_id, story_id, target, model_name, success, detail)
               values ($1, $2, $3, $4, true, $5)
               on conflict (command_id, model_name) do update
                 set success = true, detail = excluded.detail, executed_at = now()`,
              [input.commandId, input.storyId, input.target, model, 'concurrent refresh completed'],
            )
          } catch (error) {
            const detail = String((error as Error)?.message ?? error)
            await pool
              .query(
                `insert into forge_derived_refresh_execution
                   (command_id, story_id, target, model_name, success, detail)
                 values ($1, $2, $3, $4, false, $5)
                 on conflict (command_id, model_name) do update
                   set success = false, detail = excluded.detail, executed_at = now()`,
                [input.commandId, input.storyId, input.target, model, detail],
              )
              .catch(() => undefined)
            return { success: false, detail: `${model} refresh failed: ${detail}` }
          }
        }
        return { success: true, detail: `refreshed ${input.models.join(', ')}` }
      } finally {
        await pool.end()
      }
    },

    async verifyDerived(input) {
      const pool = poolForTarget(input.target)
      try {
        for (const model of input.models) {
          const name = model.includes('.') ? model.split('.')[1] : model
          const result = await pool.query(
            `select 1
             from forge_derived_refresh_execution r
             join pg_matviews mv on mv.matviewname = $4 and mv.ispopulated = true
             where r.story_id = $1 and r.target = $2 and r.model_name = $3
               and r.success = true
             order by r.executed_at desc
             limit 1`,
            [input.storyId, input.target, model, name],
          )
          if (result.rowCount !== 1) {
            return { success: false, detail: `${model} has no verified populated refresh` }
          }
        }
        return { success: true, detail: `verified ${input.models.join(', ')}` }
      } finally {
        await pool.end()
      }
    },
  }
}

export const forgeReleaseSafety = { migrationPath, quotedModel }
