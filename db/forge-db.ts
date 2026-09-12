// ---------------------------------------------------------------------------
// ForgeDB — the ONE database connection for the entire application.
//
// Captain's directive, 2026-09-12: a connection pool, and no class anywhere
// running its own raw SQL without going through it. This module is that pool. It
// is the ONLY module in the repository allowed to construct a pool or a raw
// client, and `workflow_app/tests/db-boundary.test.ts` fails the build if anything
// else tries.
//
// WHY IT EXISTS. Before this, ~40 files each opened their own connection and each
// re-decided which database to talk to — `lib/neon-interactive.ts` even carried a
// copy of the environment rule with a "keep in sync" comment, so the two copies
// could and did diverge. That is how 306 Forge runs, their board rows and their
// evidence ended up in the DEV database while the board lived in PROD, and why
// "which database am I on" had no single answer.
//
// DESIGN RULES
//   1. ONE pool per process, per declared target. Repeated calls return the same
//      pool, and nothing here runs at module load, so importing this module is
//      free and safe without a configured database.
//   2. The target is DECLARED (lib/execution-target), never inferred; silence
//      refuses. `forTarget()` exists for operator scripts that must name a target
//      explicitly, and it still resolves to the same shared pool.
//   3. Bounded by default: many short-lived instances must not open an unbounded
//      number of connections. Limits come from env with conservative defaults.
//   4. Failures reach the durable capture framework — including the pool's own
//      idle-client errors, which are otherwise an invisible hole.
// ---------------------------------------------------------------------------

import { Pool, type PoolClient, type PoolConfig } from 'pg'

import { captureError } from './app-error'
import type { QueryExecutor, QueryRow } from './query-executor'
import { declareControlPlane } from '../lib/execution-target'
import { flattenSqlTemplate, toPgQuery } from './sql-template'

export type ForgeDbTarget = 'prod' | 'dev'

export class ForgeDbConfigError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ForgeDbConfigError'
  }
}

/** Pool sizing. Small on purpose: serverless instances are many and short-lived. */
function poolConfig(target: ForgeDbTarget): PoolConfig {
  const intFromEnv = (name: string, fallback: number): number => {
    const raw = process.env[name]
    const parsed = raw ? Number.parseInt(raw, 10) : Number.NaN
    return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
  }
  return {
    max: intFromEnv('FORGE_DB_POOL_MAX', 5),
    idleTimeoutMillis: intFromEnv('FORGE_DB_POOL_IDLE_MS', 10_000),
    connectionTimeoutMillis: intFromEnv('FORGE_DB_POOL_CONNECT_MS', 10_000),
    application_name: `culebraluxe-forgedb-${target}`,
  }
}

/**
 * Pin the SSL mode explicitly.
 *
 * `pg` 8.23 prints a SECURITY WARNING on every connect because `sslmode=require`
 * (and `prefer`, `verify-ca`) are currently aliases of `verify-full` but are going
 * to change meaning. Measured against the live Neon databases before changing
 * anything: `require` and `verify-full` both connect and behave identically. So
 * pinning `verify-full` keeps today's behaviour exactly, states the intent, and
 * takes the warning out of the application's core connection path. Verification is
 * unchanged, not weakened — `rejectUnauthorized` is never disabled here.
 */
function withExplicitSslMode(url: string): string {
  if (!/\bsslmode=/.test(url)) return url
  return url.replace(/(\bsslmode=)(prefer|require|verify-ca)\b/g, '$1verify-full')
}

export function forgeDbConnectionString(
  target: ForgeDbTarget,
  env: NodeJS.ProcessEnv = process.env,
): string {
  const url = target === 'prod' ? env.DATABASE_URL_PROD : env.DATABASE_URL_DEV
  if (!url) {
    throw new ForgeDbConfigError(
      `DATABASE_URL_${target.toUpperCase()} is not configured. ForgeDB refuses to open a connection ` +
        'rather than fall back to the other environment.',
    )
  }
  return withExplicitSslMode(url)
}

const pools = new Map<ForgeDbTarget, Pool>()

/**
 * The shared pool for a target. Lazy: the first call creates it and later calls
 * return the SAME pool, so the process holds one pool per target.
 */
export function forgeDbPool(
  target?: ForgeDbTarget,
  env: NodeJS.ProcessEnv = process.env,
): Pool {
  const resolved = target ?? declareControlPlane(env).target
  const existing = pools.get(resolved)
  if (existing) return existing

  const pool = new Pool({
    connectionString: forgeDbConnectionString(resolved, env),
    ...poolConfig(resolved),
  })

  // An idle client that dies (network, Neon restart, idle timeout) emits here.
  // With no listener this becomes an unhandled 'error' event that can take the
  // process down; with a listener but no capture it is invisible. Capture it.
  pool.on('error', (error: Error) => {
    captureError({
      kind: 'DB_POOL_CLIENT_ERROR',
      operation: 'forge-db.pool',
      message: `idle pool client error (${resolved}): ${error.message}`,
      code: (error as { code?: string }).code ?? null,
      level: 'error',
      meta: { target: resolved },
    })
  })

  pools.set(resolved, pool)
  return pool
}

async function runAgainst(
  pool: Pool,
  strings: readonly string[],
  values: readonly unknown[],
): Promise<QueryRow[]> {
  const flattened = flattenSqlTemplate(strings, values)
  const query = toPgQuery(flattened.strings, flattened.values)
  const result = await pool.query(query.text, query.values)
  return result.rows as QueryRow[]
}

/**
 * Run `cb` inside an interactive transaction on ONE pooled client: a real
 * BEGIN/COMMIT/ROLLBACK on a single connection. A throw anywhere in `cb` rolls
 * back, and a rollback failure is captured rather than replacing the cause.
 */
export async function withForgeTransaction<T>(
  cb: (tx: QueryExecutor) => Promise<T>,
  target?: ForgeDbTarget,
): Promise<T> {
  const pool = forgeDbPool(target)
  const client: PoolClient = await pool.connect()
  try {
    await client.query('BEGIN')
    const result = await cb((strings: TemplateStringsArray, ...values: unknown[]) =>
      runAgainstClient(client, strings, values),
    )
    await client.query('COMMIT')
    return result
  } catch (error) {
    try {
      await client.query('ROLLBACK')
    } catch (rollbackError) {
      captureError({
        kind: 'DB_ROLLBACK_FAILED',
        operation: 'forge-db.transaction',
        message: `rollback failed: ${(rollbackError as Error)?.message ?? String(rollbackError)}`,
        level: 'error',
      })
    }
    throw error
  } finally {
    client.release()
  }
}

async function runAgainstClient(
  client: PoolClient,
  strings: readonly string[],
  values: readonly unknown[],
): Promise<QueryRow[]> {
  const flattened = flattenSqlTemplate(strings, values)
  const query = toPgQuery(flattened.strings, flattened.values)
  const result = await client.query(query.text, query.values)
  return result.rows as QueryRow[]
}

/**
 * A handle bound to an EXPLICIT target, for operator scripts that must state where
 * they run. It shares the same pool registry, so an explicit `prod` handle and the
 * declared `prod` target are the same pool — "explicit" must not mean "second
 * connection".
 */
export function forgeDbForTarget(target: ForgeDbTarget) {
  const pool = () => forgeDbPool(target)
  return {
    target,
    pool,
    sql: (strings: TemplateStringsArray, ...values: unknown[]) =>
      runAgainst(pool(), strings, values),
    /** Raw text + params, for the few places that build a text with `$n` by hand. */
    runText: (text: string, params: unknown[] = []) => {
      const p = pool()
      return p.query(text, params).then((r) => r.rows as QueryRow[])
    },
    transaction: <T>(cb: (tx: QueryExecutor) => Promise<T>) => withForgeTransaction(cb, target),
    stats: () => poolStats(target),
    end: () => endForgeDb(target),
  }
}

function poolStats(target?: ForgeDbTarget) {
  const resolved = target ?? declareControlPlane().target
  const pool = pools.get(resolved)
  return {
    target: resolved,
    created: Boolean(pool),
    totalCount: pool?.totalCount ?? 0,
    idleCount: pool?.idleCount ?? 0,
    waitingCount: pool?.waitingCount ?? 0,
  }
}

async function endForgeDb(target?: ForgeDbTarget): Promise<void> {
  const resolved = target ?? declareControlPlane().target
  const pool = pools.get(resolved)
  if (!pool) return
  pools.delete(resolved)
  await pool.end()
}

/**
 * ForgeDB — the application's single entry point for database work.
 *
 * `forgeDb.sql` is the tagged executor every caller should use; `pool()` is for
 * the rare code that needs a client (and must release it); `stats()` and `end()`
 * exist so tests and shutdown have an honest handle on the pool.
 */
export const forgeDb = {
  get target(): ForgeDbTarget {
    return declareControlPlane().target
  },
  pool: (target?: ForgeDbTarget) => forgeDbPool(target),
  /**
   * The tagged executor. Resolved PER CALL, never at module load: binding a pool
   * here would make importing this module require a declared environment and a
   * configured URL, which the rest of the codebase relies on not being true.
   */
  sql: ((strings: TemplateStringsArray, ...values: unknown[]) =>
    runAgainst(forgeDbPool(), strings, values)) as QueryExecutor,
  runText: (text: string, params: unknown[] = []) =>
    forgeDbPool()
      .query(text, params)
      .then((r) => r.rows as QueryRow[]),
  transaction: withForgeTransaction,
  forTarget: forgeDbForTarget,
  stats: () => poolStats(),
  end: () => endForgeDb(),
}

