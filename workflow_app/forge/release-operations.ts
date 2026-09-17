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
      try {
        for (const migration of migrations) {
          try {
            await pool.query(migration.sql)
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
            completed.push(migration.file)
            // Durable ledger (migration 144): the canonical record of what was
            // applied where, independent of this story's execution history.
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
          } catch (error) {
            const detail = String((error as Error)?.message ?? error)
            await pool
              .query(
                `insert into forge_migration_execution
                   (command_id, story_id, target, migration_file, content_sha256, success, detail)
                 values ($1, $2, $3, $4, $5, false, $6)
                 on conflict (command_id, migration_file) do update
                   set success = false, detail = excluded.detail, executed_at = now()`,
                [input.commandId, input.storyId, input.target, migration.file, migration.sha256, detail],
              )
              .catch(() => undefined)
            return {
              success: false,
              detail: `${migration.file} failed after [${completed.join(', ')}]: ${detail}`,
            }
          }
        }
        return { success: true, detail: `applied ${completed.join(', ')} to ${input.target}` }
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
