import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { basename, relative, resolve, sep } from 'node:path'
import { Pool } from '@neondatabase/serverless'

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
        sql,
        sha256: createHash('sha256').update(sql, 'utf8').digest('hex'),
      }
    }),
  )
}

export function createForgeReleaseOperations(): ForgeReleaseOperations {
  return {
    async applyMigrations(input) {
      const migrations = await migrationContents(input.repoRoot, input.migrationFiles)
      const pool = new Pool({ connectionString: databaseUrl(input.target) })
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
      const pool = new Pool({ connectionString: databaseUrl(input.target) })
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
        }
        await pool.query('select 1')
        return {
          success: true,
          detail: `verified ${migrations.length} checksum-matched migration execution(s) on ${input.target}`,
        }
      } finally {
        await pool.end()
      }
    },

    async refreshDerived(input) {
      if (input.models.length === 0) throw new Error('derivedRefreshRequired=true but derivedModels is empty')
      const pool = new Pool({ connectionString: databaseUrl(input.target) })
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
      const pool = new Pool({ connectionString: databaseUrl(input.target) })
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
