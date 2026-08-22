// ---------------------------------------------------------------------------
// Execution Target (ENG-20) — explicit separation of CONTROL PLANE from the
// EXECUTION TARGET.
//
//   CONTROL PLANE:   storyboard_story / agent_work_item / storyboard_story_run
//                    — the canonical queue/run/evidence tables. May live in
//                    production while a command executes against DEV/local
//                    resources.
//   EXECUTION TARGET: the environment the SDLC work actually runs against
//                    (local repository, DEV application/domain Neon, local
//                    processes, local DeepSeek Harness).
//
// The execution target is NEVER inferred from the database that stores the
// control-plane row. It is resolved explicitly (EXECUTION_ENV) and persisted
// on the durable command + run evidence so an operator can always distinguish
// "control plane: PROD" from "execution target: DEV".
//
// FAIL-FAST RULE: before any database-affecting SDLC command executes, the
// intended execution environment is resolved and the application/domain DB
// configuration is verified against it. A DEV command that would resolve to
// the PRODUCTION application/domain database — including through a generic
// DATABASE_URL fallback — is refused BEFORE external work begins.
// ---------------------------------------------------------------------------

export type ExecutionEnvironment = 'DEV' | 'PROD' | 'TEST' | 'LOCAL'

export const EXECUTION_ENVIRONMENTS: readonly ExecutionEnvironment[] = [
  'DEV',
  'PROD',
  'TEST',
  'LOCAL',
]

export const EXECUTION_ENVIRONMENT_LABELS: Record<ExecutionEnvironment, string> = {
  DEV: 'DEV / local development database',
  PROD: 'PROD / production database',
  TEST: 'TEST / test database',
  LOCAL: 'LOCAL / local development resources',
}

export class ExecutionTargetError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ExecutionTargetError'
  }
}

export function isExecutionEnvironment(value: string): value is ExecutionEnvironment {
  return (EXECUTION_ENVIRONMENTS as readonly string[]).includes(value)
}

/**
 * Parse an execution environment token. Accepts the four canonical values plus
 * the common `DEVELOPMENT` / `PRODUCTION` aliases. FAIL CLOSED: an unknown or
 * empty value throws unless an explicit fallback is supplied.
 */
export function parseExecutionEnvironment(
  raw: string | null | undefined,
  fallback?: ExecutionEnvironment,
): ExecutionEnvironment {
  const v = (raw ?? '').trim().toUpperCase()
  if (v === 'DEV' || v === 'DEVELOPMENT') return 'DEV'
  if (v === 'PROD' || v === 'PRODUCTION') return 'PROD'
  if (v === 'TEST') return 'TEST'
  if (v === 'LOCAL') return 'LOCAL'
  if (v === '' && fallback) return fallback
  throw new ExecutionTargetError(
    `execution environment is not configured: expected EXECUTION_ENV in (DEV|PROD|TEST|LOCAL), got ${JSON.stringify(raw ?? null)}`,
  )
}

/**
 * Resolve the INTENDED execution target from process configuration.
 * Precedence: explicit EXECUTION_ENV, then a conservative APP_ENV mapping,
 * then fail closed. The control-plane database is intentionally NOT consulted
 * (the execution target must never be inferred from where the row lives).
 */
export function resolveExecutionTarget(): ExecutionEnvironment {
  if (process.env.EXECUTION_ENV) {
    return parseExecutionEnvironment(process.env.EXECUTION_ENV)
  }
  const appEnv = (process.env.APP_ENV ?? 'development').trim().toLowerCase()
  if (appEnv === 'production' || appEnv === 'prod') return 'PROD'
  if (appEnv === 'development' || appEnv === 'dev' || appEnv === '') return 'DEV'
  if (appEnv === 'test' || appEnv === 'testing') return 'TEST'
  throw new ExecutionTargetError(
    `cannot resolve execution target from APP_ENV=${JSON.stringify(appEnv)}; set EXECUTION_ENV explicitly`,
  )
}

/**
 * The application/domain database URL the intended execution target SHOULD use.
 * DEV/LOCAL/TEST execution all resolve to the DEV application database;
 * PROD execution resolves to the production application database.
 */
export function databaseUrlForExecutionTarget(target: ExecutionEnvironment): string | null {
  if (target === 'PROD') return process.env.DATABASE_URL_PROD ?? null
  return process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL ?? null
}


/**
 * FAIL-FAST guard — call BEFORE any database-affecting SDLC work begins.
 *
 * Verifies the application/domain DB configuration for the intended execution
 * target:
 *   - a non-PROD target whose DATABASE_URL_DEV resolves to the PRODUCTION
 *     database is refused (a DEV command must never touch production data);
 *   - a non-PROD target whose generic DATABASE_URL fallback resolves to the
 *     PRODUCTION database is refused (a generic fallback must never silently
 *     point a DEV command at production);
 *   - a PROD target whose DATABASE_URL_PROD resolves to the DEV database is
 *     refused (production work must run against the production database).
 *
 * URL comparison is exact-string and case-sensitive, so an obviously swapped
 * or mis-typed value is caught without connecting to either database.
 */
export function assertExecutionTargetSafe(target: ExecutionEnvironment): void {
  const prodUrl = process.env.DATABASE_URL_PROD ?? null
  const devUrl = process.env.DATABASE_URL_DEV ?? null
  const genericUrl = process.env.DATABASE_URL ?? null

  if (target === 'PROD') {
    if (prodUrl && devUrl && prodUrl === devUrl) {
      throw new ExecutionTargetError(
        'execution target is PROD but DATABASE_URL_DEV resolves to the same URL as DATABASE_URL_PROD; refusing to start work (fail-fast).',
      )
    }
    if (prodUrl && genericUrl && prodUrl === genericUrl) {
      throw new ExecutionTargetError(
        'execution target is PROD but the generic DATABASE_URL fallback resolves to the same URL as DATABASE_URL_DEV; a fallback could silently point PROD work at the DEV database; refusing to start work (fail-fast).',
      )
    }
    return
  }

  if (devUrl && prodUrl && devUrl === prodUrl) {
    throw new ExecutionTargetError(
      `execution target is ${target} but DATABASE_URL_DEV resolves to the PRODUCTION database (identical to DATABASE_URL_PROD); refusing to start work (fail-fast).`,
    )
  }
  if (genericUrl && prodUrl && genericUrl === prodUrl) {
    throw new ExecutionTargetError(
      `execution target is ${target} but the generic DATABASE_URL fallback resolves to the PRODUCTION database; a fallback could silently point a ${target} command at production; refusing to start work (fail-fast).`,
    )
  }
}
