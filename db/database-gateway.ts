import { randomUUID } from 'node:crypto'
import { neon } from '@neondatabase/serverless'
import type { QueryExecutor, QueryRow } from './query-executor'

// ---------------------------------------------------------------------------
// DB-HARDEN-01 — Single application Database Gateway.
//
// EVERY application-owned database call flows through this gateway (directly,
// or via the contained `sql` tagged executor re-exported by db/client.ts).
// The gateway:
//   - resolves the connection LAZILY (importing this module never throws and
//     never loads the Neon driver — only an actual DB operation does)
//   - normalizes driver/Postgres exceptions into a typed DbFailure and returns
//     a discriminated Result — DB failure is NEVER masqueraded as empty data,
//     not-found, or a business rejection
//   - logs server-side diagnostics (incident id, operation, env, error code,
//     safe classification) with NO credentials, NO raw SQL, NO bind values
//
// This is the ONLY runtime module allowed to import @neondatabase/serverless.
// ---------------------------------------------------------------------------

// ---------- Result / error taxonomy ----------
export type DbFailureKind =
  | 'DATABASE_UNAVAILABLE'
  | 'SCHEMA_MISMATCH'
  | 'CONSTRAINT'
  | 'TIMEOUT'
  | 'UNKNOWN'

export type DbFailure = {
  kind: DbFailureKind
  operation: string
  incidentId: string
  /** Postgres error code when available (e.g. 42703 = undefined column). */
  code?: string
  /** Short, safe human detail — never raw SQL or credentials. */
  detail?: string
  retryable?: boolean
}

export type Result<T> = { ok: true; data: T } | { ok: false; error: DbFailure }

export class DbConfigError extends Error {
  readonly kind: DbFailureKind = 'DATABASE_UNAVAILABLE'
  constructor(appEnv: string) {
    super(`Database URL is not configured for APP_ENV="${appEnv}"`)
  }
}

/** Typed, normalized error thrown by the contained `sql` executor (backward
 *  compat). Repositories using the Result API never observe this — they get a
 *  typed failure instead. */
export class DbFailureError extends Error {
  readonly failure: DbFailure
  constructor(failure: DbFailure) {
    super(failure.kind)
    this.name = 'DbFailureError'
    this.failure = failure
  }
}

// ---------- error normalization ----------
function classifyCode(code: string): DbFailureKind {
  // Undefined column / undefined table — the schema-drift signature.
  if (code === '42703' || code === '42P01') return 'SCHEMA_MISMATCH'
  if (code === '57014' || code === '55P03') return 'TIMEOUT'
  // Connection / admin shutdown / too many connections.
  if (
    code.startsWith('08') ||
    code === '57P01' ||
    code === '57P03' ||
    code === '53300'
  ) {
    return 'DATABASE_UNAVAILABLE'
  }
  if (code.startsWith('23')) return 'CONSTRAINT'
  return 'UNKNOWN'
}

function normalizeError(e: unknown, operation: string): DbFailure {
  const err = e as { code?: string; message?: string; name?: string }
  const incidentId = randomUUID()
  const code = typeof err?.code === 'string' ? err.code : undefined
  const message = String(err?.message ?? '')

  if (code) {
    // Node network-style error codes (not Postgres SQLSTATE) — connection lost.
    if (
      /^(ECONNREFUSED|ENOTFOUND|ETIMEDOUT|EPIPE|EAI_AGAIN|EHOSTUNREACH|ECONNRESET)$/.test(
        code,
      )
    ) {
      return {
        kind: 'DATABASE_UNAVAILABLE',
        operation,
        incidentId,
        code,
        detail: 'connection failure',
        retryable: true,
      }
    }
    const kind = classifyCode(code)
    return {
      kind,
      operation,
      incidentId,
      code,
      detail: code === '42703' ? 'schema mismatch (column/table missing)' : undefined,
      retryable: kind === 'DATABASE_UNAVAILABLE' || kind === 'TIMEOUT',
    }
  }

  // Network/connection-style failures carry a non-Postgres code.
  if (/ECONNREFUSED|ENOTFOUND|ETIMEDOUT|EPIPE|socket|connect/i.test(message)) {
    return {
      kind: 'DATABASE_UNAVAILABLE',
      operation,
      incidentId,
      detail: 'connection failure',
      retryable: true,
    }
  }
  if (err?.name === 'DbConfigError' || err instanceof DbConfigError) {
    return {
      kind: 'DATABASE_UNAVAILABLE',
      operation,
      incidentId,
      detail: 'database not configured',
      retryable: false,
    }
  }
  return { kind: 'UNKNOWN', operation, incidentId }
}

function logFailure(failure: DbFailure): void {
  // Server-side only. NO credentials, NO raw SQL, NO bind values.
  const fields = [
    `[db:gateway] incident=${failure.incidentId}`,
    `operation=${failure.operation}`,
    `env=${appEnvLabel()}`,
    `kind=${failure.kind}`,
  ]
  if (failure.code) fields.push(`code=${failure.code}`)
  if (failure.detail) fields.push(`detail=${failure.detail}`)
  if (failure.retryable) fields.push('retryable=true')
  console.error(fields.join(' '))
}

/** Safe, coarse operation label derived from the SQL keyword ONLY (never raw
 *  SQL, never bind values). Repositories may pass an explicit operation where
 *  finer-grained diagnostics are wanted. */
function opFromTemplate(strings: TemplateStringsArray): string {
  const first = (strings[0] ?? '').trim().toLowerCase()
  const m = first.match(
    /\b(select|insert|update|delete|with|refresh|create|alter|drop|call|truncate)\b/,
  )
  return m?.[1] ?? 'sql'
}

// ---------- lazy connection ----------
export type DbTarget = 'prod' | 'dev'

/**
 * Resolve the effective database TARGET from the runtime environment.
 *
 * Vercel's built-in runtime env (`VERCEL_ENV`) is the authoritative signal for
 * hosted deployments — Vercel does NOT set APP_ENV, so keying routing off
 * APP_ENV alone previously sent EVERY deployment (including Production) to the
 * DEV database (the `env=development` seen in prod runtime logs).
 *
 * Resolution order:
 *   VERCEL_ENV=production           -> prod
 *   VERCEL_ENV=preview|development  -> dev
 *   otherwise (local / CLI / operator scripts):
 *     APP_ENV=production            -> prod
 *     otherwise                     -> dev
 */
export function resolveDbTarget(env: NodeJS.ProcessEnv = process.env): DbTarget {
  const vercelEnv = env.VERCEL_ENV
  if (vercelEnv === 'production') return 'prod'
  if (vercelEnv === 'preview' || vercelEnv === 'development') return 'dev'
  const appEnv = env.APP_ENV ?? 'development'
  return appEnv === 'production' ? 'prod' : 'dev'
}

export function getDatabaseUrl(): string {
  const target = resolveDbTarget()
  const url =
    target === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  // Fail closed: never silently use the other environment for a deployment.
  if (!url) throw new DbConfigError(target === 'prod' ? 'production' : 'development')
  return url
}

function appEnvLabel(): string {
  return resolveDbTarget() === 'prod' ? 'production' : 'development'
}

/**
 * Safe, credential-free diagnostic: the resolved database target and the Neon
 * branch token parsed from the connection HOST only (never the password/user).
 * Lets an operator confirm which branch a deployment is actually reading.
 */
export function dbTargetInfo(): {
  target: DbTarget
  vercelEnv: string | undefined
  appEnv: string | undefined
  neonBranch: string | null
} {
  const target = resolveDbTarget()
  const url =
    target === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  let neonBranch: string | null = null
  if (url) {
    try {
      const host = new URL(url).host
      // Neon branch hosts carry the branch as the leading `--` token, e.g.
      // `br-snowy-fog-axg3jae2--project-abc123...neon.tech`.
      const m = host.match(/^([a-z0-9]+(?:-[a-z0-9]+)*)--/)
      neonBranch = m ? m[1] : host
    } catch {
      neonBranch = null
    }
  }
  return {
    target,
    vercelEnv: process.env.VERCEL_ENV,
    appEnv: process.env.APP_ENV,
    neonBranch,
  }
}

type SqlExecutor = QueryExecutor

/**
 * Eager-but-safe executor. The Neon driver object is created at module load
 * ONLY when a database URL is configured (so module import never throws).
 * If configuration is missing, this is null and the gateway reports
 * DATABASE_UNAVAILABLE on the first actual operation. Creating it eagerly
 * keeps the legacy synchronous nested-fragment semantics (`sql\`...${frag}...\``)
 * intact. The driver object is stateless — no connection is opened until a
 * query actually runs.
 */
function createExecutorSafely(): SqlExecutor | null {
  try {
    return neon(getDatabaseUrl()) as unknown as SqlExecutor
  } catch {
    return null // missing/invalid config — import never throws
  }
}

const SQL_EXECUTOR: SqlExecutor | null = createExecutorSafely()

// Test-only fault-injection hook. When set, every gateway query routes through
// this executor instead of Neon, so tests can deliberately produce
// schema-mismatch / connection / timeout / constraint / generic failures.
let testExecutor: SqlExecutor | null = null

/** TEST HOOK — inject a fake/fault-injectable executor, or null to restore
 *  the real executor. Never used by application runtime. */
export function setDatabaseTestExecutor(exec: SqlExecutor | null): void {
  testExecutor = exec
}

type TransactionRunner = <T>(cb: (tx: QueryExecutor) => Promise<T>) => Promise<T>

// Test-only transaction hook (see setDatabaseTestTransaction).
let testTransaction: TransactionRunner | null = null

/** TEST HOOK — inject a fake transaction runner, or null to restore the real
 *  interactive transaction. Never used by application runtime. */
export function setDatabaseTestTransaction(runner: TransactionRunner | null): void {
  testTransaction = runner
}

/** Resolve the active executor: the injected test executor, else (DEV-only)
 *  an env-requested fault executor, else the real one (null when the database
 *  is not configured). */
function getExecutor(): SqlExecutor | null {
  if (testExecutor) return testExecutor
  const fault = devFaultCode()
  if (fault) return faultExecutorFor(fault)
  return SQL_EXECUTOR
}

/** DEV-only fault-injection for runtime isolation proofs (section 11). Set
 *  DB_HARDEN_FAULT=<postgres-code> in a NON-production environment to make
 *  every gateway query fail with that error code (e.g. 42703 = schema
 *  mismatch). Never activates in production. */
function devFaultCode(): string | null {
  if (resolveDbTarget() === 'prod') return null
  const code = process.env.DB_HARDEN_FAULT
  return code && code.length > 0 ? code : null
}

function faultExecutorFor(code: string): SqlExecutor {
  return async () => {
    const err = new Error(`fault injection (${code})`)
    ;(err as { code?: string }).code = code
    throw err
  }
}

// ---------- DatabaseGateway ----------
export class DatabaseGateway {
  /** Typed query — returns a Result. Never throws a raw driver error. */
  async query<T extends QueryRow = QueryRow>(
    strings: TemplateStringsArray,
    ...params: unknown[]
  ): Promise<Result<T[]>> {
    const operation = opFromTemplate(strings)
    try {
      const exec = getExecutor()
      if (!exec) throw new DbConfigError(appEnvLabel())
      const rows = (await exec(strings, ...params)) as T[]
      return { ok: true, data: rows }
    } catch (e) {
      const failure = normalizeError(e, operation)
      logFailure(failure)
      return { ok: false, error: failure }
    }
  }

  /** Typed single-row query — null means zero rows (NOT a failure). */
  async queryOne<T extends QueryRow = QueryRow>(
    strings: TemplateStringsArray,
    ...params: unknown[]
  ): Promise<Result<T | null>> {
    const result = await this.query<T>(strings, ...params)
    if (!result.ok) return result
    return { ok: true, data: result.data[0] ?? null }
  }

  /** Typed write/execute — returns the affected rows (for writes usually []). */
  async execute<T extends QueryRow = QueryRow>(
    strings: TemplateStringsArray,
    ...params: unknown[]
  ): Promise<Result<T[]>> {
    return this.query<T>(strings, ...params)
  }

  /**
   * Typed interactive transaction. The callback receives the raw tagged
   * executor and may throw internally to force rollback; the PUBLIC boundary
   * converts any failure (including rollback) into ONE typed failure. A
   * partially-successful transaction never masquerades as success.
   */
  async transaction<T>(
    operation: string,
    cb: (tx: QueryExecutor) => Promise<T>,
  ): Promise<Result<T>> {
    try {
      const run =
        testTransaction ??
        (await import('../lib/neon-interactive')).withTransaction
      const data = await run(cb)
      return { ok: true, data }
    } catch (e) {
      const failure = normalizeError(e, operation)
      logFailure(failure)
      return { ok: false, error: failure }
    }
  }
}

/** Application singleton gateway. */
export const db = new DatabaseGateway()

// ---------- contained backward-compat tagged executor ----------
/**
 * Contained tagged executor routed through the gateway. A Proxy over the real
 * Neon Sql object that is created eagerly-but-safely at module load, so it
 * supports the SAME tagged-template AND nested-fragment semantics as the legacy
 * executor (e.g. `sql\`...${fragment}...\``). Importing it never throws even
 * when the database URL is missing (queries then report DATABASE_UNAVAILABLE).
 * Direct usage returns rows (or throws the driver error); repositories doing
 * local containment should prefer `db.query` (typed, normalized Result).
 */
type SqlLike = {
  (strings: TemplateStringsArray, ...values: unknown[]): Promise<QueryRow[]>
  [key: string]: unknown
}

// ---------- contained backward-compat tagged executor ----------
/**
 * Contained tagged executor routed through the gateway. Returns rows and
 * throws a NORMALIZED DbFailureError on failure (never a raw driver error) —
 * no raw Neon/Postgres exception can escape application code. Repositories
 * doing local containment should prefer `db.query` (typed Result). Structural
 * SQL fragments (e.g. constant ORDER BY / WHERE) must use the gateway `raw`
 * builder, not this executor.
 */
export async function sql(
  strings: TemplateStringsArray,
  ...params: unknown[]
): Promise<QueryRow[]> {
  const result = await db.query(strings, ...params)
  if (!result.ok) throw new DbFailureError(result.error)
  return result.data as QueryRow[]
}

/**
 * Safe structural-SQL fragment builder. Produces a real Neon fragment for
 * CONSTANT, non-user-controlled SQL (e.g. a fixed ORDER BY clause) so it can be
 * interpolated into a `db.query`/`sql` template. The fragment's bind values
 * (if any) are still parameterized. Never pass user input here.
 *
 * Building a fragment is a pure structural operation and MUST NOT require a
 * live executor: repositories define constant fragments at module scope, and an
 * eager executor resolution there makes importing the module depend on a
 * configured database. The fragment is flattened + bound by the parent query's
 * executor (Neon or the gateway) when it is interpolated, which is where the
 * "database not configured" failure correctly surfaces.
 */
export function raw(strings: TemplateStringsArray, ...values: unknown[]) {
  return { strings, values };
}

function configFailure(): DbFailure {
  return {
    kind: 'DATABASE_UNAVAILABLE',
    operation: 'sql',
    incidentId: randomUUID(),
    detail: 'database not configured',
    retryable: false,
  }
}

