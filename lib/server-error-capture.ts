// -----------------------------------------------------------------------------
// Server error capture seam — severity-aware (INFO/WARN/ERROR/FATAL) writing to
// the durable app_error table. Call this in a server action's catch block or a
// route handler's catch block. It is fire-and-forget (never throws into the
// operation being recorded) and never logs secrets/raw SQL/bind values.
//
//   try { ... }
//   catch (e) { throw captureServerError('claim:activate', e, { route: '/x' }) }
//   // cleanup (release/rollback) belongs in finally, not here.
// -----------------------------------------------------------------------------
import { captureError, type ErrorLevel } from '../db/app-error'

export type CaptureOpts = {
  level?: ErrorLevel
  storyId?: string | null
  route?: string | null
}

const LEVEL_PREFIX: Record<ErrorLevel, string> = {
  info: '[server:info]',
  warn: '[server:warn]',
  error: '[server:error]',
  fatal: '[server:fatal]',
}

function defaultLevel(err: unknown): ErrorLevel {
  const name = err instanceof Error ? err.name : typeof err
  if (/fatal/i.test(name)) return 'fatal'
  if (err instanceof Error && /warn/i.test(name)) return 'warn'
  return 'error'
}

/** Capture an error durably (best-effort) and return it so the caller can
 * rethrow or handle. `level` defaults to error (fatal/warn inferred by name). */
export function captureServerError(label: string, err: unknown, opts: CaptureOpts = {}): Error {
  const level = opts.level ?? defaultLevel(err)
  const error = err instanceof Error ? err : new Error(String(err))
  const message = error.message.trim() || label
  console.error(`${LEVEL_PREFIX[level]} ${label}: ${message}`)
  captureError({
    kind: error.name || 'Error',
    operation: label,
    message,
    stack: error.stack ?? null,
    storyId: opts.storyId ?? null,
    route: opts.route ?? null,
    level,
  })
  return error
}

/** Non-throwing, structured log line for info/warn observations (no exception). */
export function captureServerLog(level: 'info' | 'warn', label: string, message: string, opts: CaptureOpts = {}): void {
  const text = message.trim() || label
  const marker = level === 'info' ? LEVEL_PREFIX.info : LEVEL_PREFIX.warn
  console.log(`${marker} ${label}: ${text}`)
  captureError({
    kind: level === 'info' ? 'INFO' : 'WARN',
    operation: label,
    message: text,
    storyId: opts.storyId ?? null,
    route: opts.route ?? null,
    level,
  })
}
