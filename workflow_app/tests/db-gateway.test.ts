// ---------------------------------------------------------------------------
// DB-HARDEN-01 — Database Gateway failure containment + Neon-import rule.
//
//   A. Property-collection SQL throws        -> typed failure, no escape
//   B/C/D. Read failures                     -> typed failure (not 404/empty)
//   E. Valid query, zero rows                -> ok/empty, distinguishable
//   F. Write fails                           -> no success, typed failure
//   G. Transaction fails midway              -> rollback, one typed failure
//   H. Database URL absent                   -> module import ok; op returns
//                                               DATABASE_UNAVAILABLE
//   ARCH. Only the gateway (+ interactive tx substrate) imports Neon.
// ---------------------------------------------------------------------------

import { test, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'

import {
  db,
  sql,
  DbFailureError,
  setDatabaseTestExecutor,
  setDatabaseTestTransaction,
} from '../../db/client'
import { getSecurityPrincipal } from '../../db/auth-user'
import type { QueryExecutor } from '../../db/query-executor'

function faultExecutor(fail: () => Promise<unknown>): QueryExecutor {
  return async () => {
    throw await fail()
  }
}

function pgError(code: string, message: string) {
  const e = new Error(message)
  ;(e as { code?: string }).code = code
  return e
}

afterEach(() => {
  setDatabaseTestExecutor(null)
  setDatabaseTestTransaction(null)
})

test('Test A: property collection SQL throws -> typed failure, no exception escapes', async () => {
  setDatabaseTestExecutor(
    faultExecutor(async () => pgError('42703', 'column p.bogus does not exist')),
  )
  const r = await db.query`select * from property`
  assert.equal(r.ok, false)
  if (!r.ok) {
    assert.equal(r.error.kind, 'SCHEMA_MISMATCH')
    assert.equal(r.error.code, '42703')
    assert.ok(r.error.incidentId, 'incident id present')
    assert.ok(!('sql' in r.error), 'no SQL leaked into the failure')
  }
})

test('Test B/C/D: read failures are typed DATABASE_UNAVAILABLE (not 404, not empty)', async () => {
  setDatabaseTestExecutor(faultExecutor(async () => pgError('ECONNREFUSED', 'connect failed')))
  const collection = await db.query`select * from property`
  assert.equal(collection.ok, false)
  if (!collection.ok) assert.equal(collection.error.kind, 'DATABASE_UNAVAILABLE')

  const detail = await db.queryOne`select * from property where slug = 'x'`
  assert.equal(detail.ok, false, 'detail failure is NOT null and NOT 404')
  if (!detail.ok) assert.equal(detail.error.kind, 'DATABASE_UNAVAILABLE')
})

test('Test E: valid query with zero rows is ok/empty — distinguishable from failure', async () => {
  setDatabaseTestExecutor(async () => [])
  const rows = await db.query`select id from property where 1 = 0`
  assert.equal(rows.ok, true)
  if (rows.ok) assert.deepEqual(rows.data, [])

  const one = await db.queryOne`select id from property where 1 = 0`
  assert.equal(one.ok, true)
  if (one.ok) assert.equal(one.data, null)
})

test('Test F: write failure -> no success, typed failure', async () => {
  setDatabaseTestExecutor(faultExecutor(async () => pgError('23505', 'duplicate key')))
  const r = await db.execute`update property set name = 'x' where id = 'y'`
  assert.equal(r.ok, false)
  if (!r.ok) assert.equal(r.error.kind, 'CONSTRAINT')
})

test('Test G: transaction failure midway -> rollback + one typed failure, no success', async () => {
  let queries = 0
  let rolledBack = false
  const txExecutor: QueryExecutor = async () => {
    queries++
    if (queries > 1) throw pgError('23505', 'boom on second query')
    return []
  }
  setDatabaseTestTransaction(async (cb) => {
    try {
      await cb(txExecutor)
    } catch {
      rolledBack = true
      throw pgError('23505', 'rolled back')
    }
  })
  const r = await db.transaction('deal.tx', async (tx) => {
    await tx`insert into a`
    await tx`insert into b` // second query fails -> rollback
    return 'committed'
  })
  assert.equal(r.ok, false, 'no false success')
  assert.equal(rolledBack, true, 'transaction rolled back')
  if (!r.ok) assert.equal(r.error.kind, 'CONSTRAINT')
})

test('Test G2: callback throw propagates as one typed failure after rollback', async () => {
  let rolledBack = false
  const txExecutor: QueryExecutor = async () => {
    throw new Error('cb boom')
  }
  setDatabaseTestTransaction(async (cb) => {
    try {
      await cb(txExecutor)
    } catch {
      rolledBack = true
      throw new Error('rolled back')
    }
  })
  const r = await db.transaction('x.tx', async (tx) => {
    await tx`select 1`
    return 'a'
  })
  assert.equal(r.ok, false)
  assert.equal(rolledBack, true)
})

test('Test H: database URL absent -> module import does not throw; op returns DATABASE_UNAVAILABLE', () => {
  // Run in a subprocess with DATABASE_URL_DEV unset so the gateway imports
  // fresh (missing config at import time). Import must not throw; the first
  // actual query must return DATABASE_UNAVAILABLE.
  const code = `
    import { db } from './db/client'
    const r = await db.query\`select 1\`
    console.log('RESULT=' + (r.ok ? 'ok' : r.error.kind))
  `
  const res = spawnSync(
    process.execPath,
    ['--import', 'tsx', '--input-type=module', '-e', code],
    {
      cwd: process.cwd(),
      env: { ...process.env, APP_ENV: 'development', DATABASE_URL_DEV: '' },
      encoding: 'utf8',
    },
  )
  assert.ok(res.status === 0, `module import must not throw: ${res.stderr}`)
  assert.match(res.stdout, /RESULT=DATABASE_UNAVAILABLE/)
})

test('contained sql executor throws a NORMALIZED DbFailureError (no raw driver error)', async () => {
  setDatabaseTestExecutor(faultExecutor(async () => pgError('42703', 'bad column')))
  await assert.rejects(
    () => sql`select * from property`,
    (err: unknown) =>
      err instanceof DbFailureError && err.failure.kind === 'SCHEMA_MISMATCH',
  )
})

import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

test('Test AUTH-FAIL-CLOSED: getSecurityPrincipal returns null (denies) on DB failure, no exception escapes', async () => {
  setDatabaseTestExecutor(
    faultExecutor(async () => pgError('42703', 'schema drift in auth projection')),
  )
  const principal = await getSecurityPrincipal('u-1')
  assert.equal(principal, null, 'auth fails closed: no principal on DB failure')
})

test('ARCH: runtime application code must not import the Neon driver outside the gateway', async () => {
  const root = fileURLToPath(new URL('../../', import.meta.url))
  const dirs = ['db', 'lib', 'app']
  const ALLOWED = new Set([
    'db/database-gateway.ts', // the one gateway
    'lib/neon-interactive.ts', // lazy interactive transaction substrate
  ])
  const violations: string[] = []
  for (const dir of dirs) {
    await walkDir(join(root, dir), (rel, source) => {
      if (rel.endsWith('.test.ts')) return
      if (ALLOWED.has(rel)) return
      if (
        /(?:from\s+|import\(\s*)[\s"']@neondatabase\/serverless[\s"']/.test(
          source,
        )
      ) {
        violations.push(rel)
      }
    })
  }
  assert.deepEqual(
    violations,
    [],
    `non-gateway runtime modules importing Neon: ${violations.join(', ')}`,
  )
})

async function walkDir(
  dir: string,
  visit: (rel: string, source: string) => void,
) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const abs = join(dir, entry.name)
    if (entry.isDirectory()) {
      await walkDir(abs, visit)
    } else if (/\.(ts|tsx)$/.test(entry.name)) {
      const source = await readFile(abs, 'utf8')
      visit(relFrom(abs), source)
    }
  }
}

function relFrom(abs: string): string {
  const parts = abs.split('/')
  for (const marker of ['db', 'lib', 'app']) {
    const idx = parts.lastIndexOf(marker)
    if (idx !== -1) return parts.slice(idx).join('/')
  }
  return abs
}
