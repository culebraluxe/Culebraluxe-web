#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Apply a migration SQL file to the control-plane database (DEV or PROD).
//
// The Neon HTTP driver cannot execute DDL, so this uses the WebSocket Pool
// (the established migration process — see docs/agent/RUNLOG.md). Multi-
// statement migration files are executed as a single simple query.
//
// Usage:
//   node --env-file=.env.local scripts/apply-migration.mjs <sql-file> [prod|dev]
//
// Default target follows APP_ENV (production -> prod, otherwise dev).
// ---------------------------------------------------------------------------
import { readFile } from 'node:fs/promises'
import { Pool } from '@neondatabase/serverless'

const file = process.argv[2]
if (!file) {
  console.error('usage: apply-migration <sql-file> [prod|dev]')
  process.exit(2)
}
const which = (
  process.argv[3] ?? (process.env.APP_ENV === 'production' ? 'prod' : 'dev')
).toLowerCase()
if (which !== 'prod' && which !== 'dev') {
  console.error(`unknown migration target: ${which}`)
  process.exit(2)
}
const url = which === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
if (!url) {
  console.error(`no ${which.toUpperCase()} DATABASE_URL configured for migrations`)
  process.exit(2)
}

const sql = await readFile(file, 'utf8')
const pool = new Pool({ connectionString: url })
try {
  await pool.query(sql)
  console.log(`applied ${file} -> ${which} control plane`)
} finally {
  await pool.end()
}
