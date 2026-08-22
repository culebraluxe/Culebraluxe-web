// ---------------------------------------------------------------------------
// CRM-08 — Google Calendar adapter: provider error model.
//
// Every provider failure is classified retryable/non-retryable so the client
// only retries genuinely transient failures (capped backoff, never a retry
// storm), and so callers fail closed on anything unknown.
//
//   retryable      — network failure / timeout / HTTP 408, 429, 5xx
//   non-retryable  — HTTP 4xx (validation, auth, not-found) and anything
//                    unknown (fail closed: never retry what we cannot
//                    classify as transient).
//
// Error messages NEVER contain credentials or tokens: the client builds them
// from the HTTP status + a short response-body excerpt only.
// ---------------------------------------------------------------------------

export const RETRYABLE_HTTP_STATUSES: ReadonlySet<number> = new Set([
  408, 429, 500, 502, 503, 504,
])

export function isTransientHttpStatus(status: number): boolean {
  return RETRYABLE_HTTP_STATUSES.has(status)
}

export type GoogleCalendarErrorClassification = {
  retryable: boolean
  message: string
}

/** Provider error carrying its own retryable classification. */
export class GoogleCalendarProviderError extends Error {
  readonly retryable: boolean
  readonly status: number | null

  constructor(message: string, status: number | null = null, retryable?: boolean) {
    super(message)
    this.name = 'GoogleCalendarProviderError'
    this.status = status
    this.retryable = retryable ?? (status !== null && isTransientHttpStatus(status))
  }
}

/**
 * Classify ANY thrown value. GoogleCalendarProviderError carries its own
 * classification; fetch network/timeout failures (TypeError/AbortError) are
 * transient; everything unknown fails closed to non-retryable.
 */
export function classifyGoogleCalendarError(
  err: unknown,
): GoogleCalendarErrorClassification {
  if (err instanceof GoogleCalendarProviderError) {
    return { retryable: err.retryable, message: err.message }
  }
  if (err instanceof Error) {
    if (
      err.name === 'AbortError' ||
      err.name === 'TimeoutError' ||
      err.name === 'TypeError'
    ) {
      return { retryable: true, message: err.message }
    }
    return { retryable: false, message: err.message }
  }
  return { retryable: false, message: String(err) }
}
