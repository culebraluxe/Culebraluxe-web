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

import { readFileSync } from 'node:fs'

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

/**
 * Build the environment for a SPAWNED child/runtime process for the intended
 * execution target (ENG-20A DEV-safety). A DEV execution MUST NEVER silently
 * resolve persistence/application tests to PROD:
 *   - the target is first asserted safe (fail fast on any DEV->PROD URL
 *     mismatch BEFORE the child is spawned);
 *   - for non-PROD targets the PROD application URL is REMOVED from the child
 *     environment and APP_ENV / EXECUTION_ENV / DATABASE_URL / DATABASE_URL_DEV
 *     are forced to the DEV application DB, so even inherited .env.local or a
 *     generic DATABASE_URL fallback cannot resolve tests to PROD;
 *   - for a PROD target the child resolves the PROD application DB explicitly.
 * No recovery by guessing another DATABASE_URL: a mismatch throws.
 */
export function buildChildProcessEnv(
  target: ExecutionEnvironment,
  baseEnv: NodeJS.ProcessEnv = process.env,
): Record<string, string | undefined> {
  assertExecutionTargetSafe(target)

  const env: Record<string, string | undefined> = { ...baseEnv }
  if (target === 'PROD') {
    env.APP_ENV = 'production'
    env.EXECUTION_ENV = 'PROD'
    if (process.env.DATABASE_URL_PROD) env.DATABASE_URL = process.env.DATABASE_URL_PROD
    return env
  }

  const devUrl =
    process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL ?? null
  env.APP_ENV = 'development'
  env.EXECUTION_ENV = target
  if (devUrl) {
    env.DATABASE_URL_DEV = devUrl
    env.DATABASE_URL = devUrl
  }
  // A DEV child must not even see the PROD application URL.
  delete env.DATABASE_URL_PROD
  return env
}

/** Minimal .env file line parser (KEY=value, optional export prefix, # comments). */
export function parseEnvFile(
  content: string,
): Record<string, string> {
  const out: Record<string, string> = {}
  for (const raw of content.split('\n')) {
    const line = raw.trim()
    if (!line || line.startsWith('#')) continue
    const body = line.replace(/^export\s+/, '')
    const eq = body.indexOf('=')
    if (eq <= 0) continue
    const key = body.slice(0, eq).trim()
    let value = body.slice(eq + 1).trim()
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1)
    }
    if (key) out[key] = value
  }
  return out
}

/**
 * Verify the WORKSPACE `.env.local` (the config any spawned test process would
 * read) against the intended execution target. FAILS FAST if a DEV execution
 * could resolve persistence/application tests to the PROD database through the
 * workspace file (APP_ENV=production, DATABASE_URL_DEV==DATABASE_URL_PROD, or a
 * generic DATABASE_URL fallback equal to the PROD url). Missing file = nothing
 * to verify (the sanitized child env already forces DEV resolution).
 */
export function verifyWorkspaceEnvFile(
  workspacePath: string,
  target: ExecutionEnvironment,
  readFile?: (path: string) => string | null,
): void {
  const read =
    readFile ??
    ((path: string): string | null => {
      try {
        return readFileSync(path, 'utf8')
      } catch {
        return null
      }
    })

  const content = read(`${workspacePath}/.env.local`)
  if (!content) return
  const fileEnv = parseEnvFile(content)

  if (target !== 'PROD') {
    const appEnv = (fileEnv.APP_ENV ?? 'development').trim().toLowerCase()
    if (appEnv === 'production' || appEnv === 'prod') {
      throw new ExecutionTargetError(
        `execution target is ${target} but the workspace .env.local sets APP_ENV=${JSON.stringify(fileEnv.APP_ENV)}; a spawned test process could resolve the PRODUCTION application database; refusing to launch (fail-fast).`,
      )
    }
    const fileDev = fileEnv.DATABASE_URL_DEV ?? null
    const fileProd = fileEnv.DATABASE_URL_PROD ?? null
    if (fileDev && fileProd && fileDev === fileProd) {
      throw new ExecutionTargetError(
        `execution target is ${target} but the workspace .env.local resolves DATABASE_URL_DEV to the PRODUCTION database; refusing to launch (fail-fast).`,
      )
    }
    const fileGeneric = fileEnv.DATABASE_URL ?? null
    if (fileGeneric && fileProd && fileGeneric === fileProd) {
      throw new ExecutionTargetError(
        `execution target is ${target} but the workspace .env.local generic DATABASE_URL resolves to the PRODUCTION database; a fallback could silently point a ${target} command at production; refusing to launch (fail-fast).`,
      )
    }
  }
}
