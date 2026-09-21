// Shared helpers for the live checks. Reads .env.local; never prints a secret.
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import { Client } from 'pg'

const here = dirname(fileURLToPath(import.meta.url))
export const repoRoot = resolve(here, '../..')

function envFile() {
  return readFileSync(resolve(repoRoot, '.env.local'), 'utf8')
}

export function envValue(name) {
  const line = envFile()
    .split('\n')
    .find((row) => row.startsWith(`${name}=`))
  return line ? line.slice(name.length + 1).trim().replace(/^["']|["']$/g, '') : undefined
}

/** The internal key the API expects: explicit env wins, otherwise derived from AUTH_SECRET. */
export function internalKey() {
  const explicit = envValue('CULEBRA_INTERNAL_API_KEY')
  if (explicit && explicit.length >= 16) return explicit
  return createHash('sha256')
    .update('culebraluxe-rust-bridge:v1:')
    .update(envValue('AUTH_SECRET') ?? '')
    .digest('hex')
}

export const apiBase = process.env.RUST_API_BASE_URL ?? 'http://127.0.0.1:8080'

/** A real mapped identity from DEV, so attributed calls can be exercised. */
export async function devIdentity() {
  const db = new Client({ connectionString: envValue('DATABASE_URL_DEV') })
  await db.connect()
  const { rows } = await db.query(
    `select provider, provider_subject from auth_identity where provider_subject is not null order by provider asc limit 1`,
  )
  await db.end()
  return rows[0]
}

export async function devDb() {
  const db = new Client({ connectionString: envValue('DATABASE_URL_DEV') })
  await db.connect()
  return db
}

export function median(xs) {
  const sorted = [...xs].sort((a, b) => a - b)
  return sorted[Math.floor(sorted.length / 2)]
}
