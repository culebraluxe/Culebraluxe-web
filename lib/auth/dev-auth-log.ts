// AUTH-08F — permanent DEV-only structured auth diagnostics.
//
// Emits a stable, greppable log line per auth stage so the DEV flow can be
// traced end-to-end without exposing secrets. Logs ONLY in non-production.
// Never logs client secret, AUTH_SECRET, tokens, cookies, JWTs, codes, or raw
// claims — only stage identifiers and safe reason codes.

function isDev(): boolean {
  return (
    process.env.NODE_ENV !== 'production' &&
    (process.env.APP_ENV ?? 'development') !== 'production'
  )
}

/**
 * Record an auth-flight event.
 *   devAuthLog('AUTH_SIGNIN_STARTED')           -> AUTH_SIGNIN_STARTED (informational)
 *   devAuthLog('GOOGLE_CALLBACK', 'INVALID_STATE') -> AUTH_ERROR stage=GOOGLE_CALLBACK reason=INVALID_STATE (error)
 *
 * Severity contract: NORMAL / SUCCESS lifecycle markers are informational
 * (console.info) so Next.js DEV never renders them as a red Console Error
 * overlay. Actual FAILURES (a reason code is present) use console.error. Both
 * channels only ever carry safe stage identifiers / reason codes — never
 * secrets, tokens, or raw claims.
 */
export function devAuthLog(stage: string, reason?: string): void {
  if (!isDev()) return
  if (reason) {
    console.error(`[auth-flight] AUTH_ERROR stage=${stage} reason=${reason}`)
    return
  }
  console.info(`[auth-flight] ${stage}`)
}

/** True when DEV diagnostics are active (used by the DEV error page). */
export function authDiagnosticsActive(): boolean {
  return isDev()
}
