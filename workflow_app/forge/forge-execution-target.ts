// ---------------------------------------------------------------------------
// ENG-FORGE-SYNC-GUARD-01 — where a Forge lane may run, decided in ONE place.
//
// Lifted out of forge-board-sync.ts, which owned this guard for the BOARD SYNC
// only. That guard protected the sync script and nothing else: the engine and the
// role runner could still launch a lane whenever the environment was quiet,
// because both defaulted the target to 'DEV' (scripts/forge-engine-worker.ts
// passed `process.env.EXECUTION_ENV ?? 'DEV'`, and the runner parsed it with a
// 'DEV' fallback). So the rule was enforced on the path that reports, never on
// the path that runs. Every Forge entry point now asks this module.
//
// THE RULE (captain, 2026-09-11, restated 2026-09-12): all Forge execution is
// PROD — prod stories, prod database, prod logs. DEV is for developing the
// engine and is expected to be discarded; a Forge lane that lands there is a
// DEFECT, not a configuration. Why it is not merely tidy: the WS series ran in
// DEV while the board lived in PROD, so shipping twelve stories left the PROD
// board unable to show its own numbers, and nothing failed to say so.
//
// Pure: it reads only the environment handed to it, so it unit-tests without a
// database and can never become a second source of routing truth.
// ---------------------------------------------------------------------------

import type { ExecutionEnvironment } from '../../lib/execution-target'

/** Forge executes against PROD. This is a rule, not a default. */
export const FORGE_EXECUTION_ENVIRONMENT: ExecutionEnvironment = 'PROD'

export class ForgeEnvironmentError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ForgeEnvironmentError'
  }
}

/**
 * Fail closed: a Forge run may only execute against PROD. `TEST` is permitted
 * only when the caller explicitly declares it a test context, so unit tests can
 * exercise the guard without pretending to be production.
 *
 * Aliases match the canonical seam in lib/execution-target.ts (PRODUCTION and
 * DEVELOPMENT are accepted there, so they are accepted here).
 */
export function assertForgeExecutionTarget(
  target: string | null | undefined,
  options?: { allowEnvironment?: ExecutionEnvironment },
): ExecutionEnvironment {
  const normalized = normalizeExecutionTarget(target)
  if (normalized === FORGE_EXECUTION_ENVIRONMENT) return 'PROD'
  if (options?.allowEnvironment && normalized === options.allowEnvironment) {
    return options.allowEnvironment
  }
  throw new ForgeEnvironmentError(
    `Forge runs against PROD only: resolved execution target was ${JSON.stringify(target ?? null)}. ` +
      'Refusing to launch (fail closed). See docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md section 0.',
  )
}

/** Canonicalize a raw execution-environment token; unknown values stay as-is. */
export function normalizeExecutionTarget(
  target: string | null | undefined,
): string {
  const raw = (target ?? '').trim().toUpperCase()
  if (raw === 'PRODUCTION') return 'PROD'
  if (raw === 'DEVELOPMENT') return 'DEV'
  return raw
}

/**
 * The target a Forge lane WOULD run in, asserted PROD.
 *
 * Precedence mirrors lib/execution-target.ts (EXECUTION_ENV, then APP_ENV) with
 * one deliberate difference: that resolver maps an absent APP_ENV to 'DEV',
 * which is the right default for application code and the wrong one here. Forge
 * gets no implicit target — silence is refused.
 */
export function resolveForgeExecutionTarget(
  env: NodeJS.ProcessEnv = process.env,
): ExecutionEnvironment {
  return assertForgeExecutionTarget(env.EXECUTION_ENV ?? env.APP_ENV ?? null)
}

/**
 * The single gate every Forge lane start passes through, BEFORE a task is
 * claimed and before any OpenCode spawn or worktree provision.
 *
 * It answers two questions, because a lane can be wrong in two directions and
 * the failure looks identical either way:
 *
 *   1. the EXECUTION target must be PROD — the worker, the work item and the
 *      OpenCode run all name the environment they ran in;
 *   2. the resolved CONTROL-PLANE database must be PROD — a lane that runs PROD
 *      resources while reading and writing the DEV board produces evidence
 *      nobody can trust, which is the WS-series failure in mirror image.
 *
 * `controlPlane` is passed in rather than imported so this stays pure: callers
 * supply `resolveDbTarget()` from db/client, which is the canonical answer to
 * "which database did this process actually resolve".
 */
export function assertForgeLaneMayStart(input: {
  env: NodeJS.ProcessEnv
  /** The control-plane database this process resolved (db/client resolveDbTarget()). */
  controlPlane: string
}): ExecutionEnvironment {
  const target = resolveForgeExecutionTarget(input.env)
  const control = normalizeExecutionTarget(input.controlPlane)
  if (control !== FORGE_EXECUTION_ENVIRONMENT) {
    throw new ForgeEnvironmentError(
      `Forge control plane must be PROD: this process resolved its database to ${JSON.stringify(
        input.controlPlane,
      )}. A lane that runs PROD resources while writing the DEV board produces evidence nobody can trust. ` +
        'Run with APP_ENV=production (see docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md section 0).',
    )
  }
  return target
}

