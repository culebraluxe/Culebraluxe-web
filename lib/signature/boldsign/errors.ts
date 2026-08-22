// ---------------------------------------------------------------------------
// DOC-04 — BoldSign Integration: provider error model.
//
// Every provider failure is classified retryable/non-retryable so the seam can
// map it to neutral 'error' with an observable classification, and so the
// client only retries genuinely transient failures (capped backoff, never a
// retry storm).
//
//   retryable      — network failure / timeout / HTTP 408, 429, 5xx
//   non-retryable  — HTTP 4xx (validation, auth, not-found) and anything
//                    unknown (fail closed: never retry what we cannot
//                    classify as transient).
//
// Error messages NEVER contain credentials: the client builds them from the
// HTTP status + a short response-body excerpt only.
// ---------------------------------------------------------------------------

export const RETRYABLE_HTTP_STATUSES: ReadonlySet<number> = new Set([
  408, 429, 500, 502, 503, 504,
])

export function isTransientHttpStatus(status: number): boolean {
  return RETRYABLE_HTTP_STATUSES.has(status)
}

export type BoldSignErrorClassification = {
  retryable: boolean
  message: string
}

/** Provider error carrying its own retryable classification. */
export class BoldSignProviderError extends Error {
  readonly retryable: boolean
  readonly status: number | null

  constructor(message: string, status: number | null = null, retryable?: boolean) {
    super(message)
    this.name = 'BoldSignProviderError'
    this.status = status
    this.retryable = retryable ?? (status !== null && isTransientHttpStatus(status))
  }
}

/**
 * Classify ANY thrown value. BoldSignProviderError carries its own
 * classification; fetch network/timeout failures (TypeError/AbortError) are
 * transient; everything unknown fails closed to non-retryable.
 */
export function classifyBoldSignError(err: unknown): BoldSignErrorClassification {
  if (err instanceof BoldSignProviderError) {
    return { retryable: err.retryable, message: err.message }
  }
  if (err instanceof Error) {
    if (err.name === 'AbortError' || err.name === 'TimeoutError' || err.name === 'TypeError') {
      return { retryable: true, message: err.message }
    }
    return { retryable: false, message: err.message }
  }
  return { retryable: false, message: String(err) }
}
