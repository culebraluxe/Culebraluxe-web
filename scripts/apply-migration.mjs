#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Apply a migration SQL file to a control-plane database (DEV or PROD), and
// RECORD it in the schema_migration ledger (migration 144).
//
// The Neon HTTP driver cannot execute DDL, so this uses the WebSocket Pool (the
// established migration process — see docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md).
// Multi-statement migration files are executed as a single simple query.
//
// Usage:
//   node --import tsx --env-file=.env.local --env-file=.env.local scripts/apply-migration.mjs <sql-file> [prod|dev] [--force] [--note "…"]
//
// Default target follows APP_ENV (production -> prod, otherwise dev).
//
// Guards:
//   * already recorded with the SAME checksum -> skipped (exit 0)
//   * already recorded with a DIFFERENT checksum -> refused (file changed after apply)
//   * --force overrides both
// ---------------------------------------------------------------------------
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { forgeDb, forgeDbTargetForUrl } from '../db/forge-db.ts'

const args = process.argv.slice(2)
const file = args.find((a) => !a.startsWith('--') && a !== 'dev' && a !== 'prod')
const force = args.includes('--force')
const noteIdx = args.indexOf('--note')
const note = noteIdx >= 0 ? args[noteIdx + 1] ?? null : null

if (!file) {
  console.error('usage: apply-migration <sql-file> [prod|dev] [--force] [--note "…"]')
  process.exit(2)
}
const which = (args.find((a) => a === 'prod' || a === 'dev') ?? (process.env.APP_ENV === 'production' ? 'prod' : 'dev')).toLowerCase()
const url = which === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
if (!url) {
  console.error(`no ${which.toUpperCase()} DATABASE_URL configured for migrations`)
  process.exit(2)
}

const sql = await readFile(file, 'utf8')
const checksum = `sha256:${createHash('sha256').update(sql).digest('hex')}`
const pool = forgeDb.forTarget(forgeDbTargetForUrl(url))

try {
  let existing
  try {
    existing = (
      await pool.query('select checksum, applied_at from schema_migration where filename = $1 and target = $2', [file, which])
    ).rows[0]
  } catch (error) {
    if (String(error?.code) === '42P01') {
      console.error('schema_migration ledger is missing — apply db/migrations/144_schema_migration_ledger.sql first')
      process.exit(2)
    }
    throw error
  }

  if (existing && !force) {
    if (existing.checksum === checksum) {
      console.log(`already applied ${file} -> ${which} (checksum match) — skipped`)
      process.exit(0)
    }
    console.error(`REFUSED: ${file} is already recorded for ${which} with a DIFFERENT checksum.`)
    console.error(`  recorded: ${existing.checksum}`)
    console.error(`  current : ${checksum}`)
    console.error('  the file changed after it was applied — review, then re-run with --force')
    process.exit(1)
  }

  await pool.query(sql)

  await pool.query(
    `insert into schema_migration (filename, checksum, target, note)
     values ($1, $2, $3, $4)
     on conflict (filename, target)
     do update set checksum = excluded.checksum, applied_at = now(), note = excluded.note`,
    [file, checksum, which, note],
  )

  console.log(`applied ${file} -> ${which} control plane (recorded in schema_migration)`)
} finally {
  await pool.end()
}
