// ---------------------------------------------------------------------------
// DIAGNOSTIC THROTTLE — the bound the anonymous diagnostic routes were missing.
//
// `/api/portal/move-trace` and `/api/portal/client-error` are deliberately
// anonymous: they exist to record failures that an authenticated path could not
// (the failure may BE an auth or database failure). Anonymous also means anyone
// can POST to them, so each must bound what it will write: per source per window,
// and per body size. Over the bound it refuses with a plain response and records
// the refusal as ONE aggregate count per window, never a row per attempt — a
// write per refused attempt would be the flood it is meant to stop.
//
// ANONYMITY IS A DESIGN INVARIANT, NOT A POLICY. The source token is used in
// memory to bound a source and is NEVER threaded into the recorded row: the
// record carries only the reason and the count. There is no field here that
// could leak it.
//
// State is per serverless instance. That bounds one instance, not a fleet;
// global/distributed limiting is an explicit non-goal of this story.
// ---------------------------------------------------------------------------

export type DiagnosticPolicy = {
  /** Length of the per-source window in milliseconds. */
  windowMs: number
  /** Writes allowed per source per window; the next one is refused. */
  maxWrites: number
  /** Bodies larger than this are refused before any write. */
  maxBodyBytes: number
}

export type DiagnosticRefusalReason = 'rate' | 'size'

export type DiagnosticVerdict =
  | { allowed: true }
  | { allowed: false; reason: DiagnosticRefusalReason; status: 429 | 413 }

export type DiagnosticRefusalReport = {
  count: number
  reason: DiagnosticRefusalReason
}

export type DiagnosticRefusalRecord = {
  kind: string
  operation: string
  message: string
  route: string
  level: 'warn'
}

export type DiagnosticThrottle = {
  /** Decide one write. Pure with respect to the caller's `now` — injectable for tests. */
  checkDiagnosticWrite(input: {
    source: string
    bodyBytes: number
    now: number
  }): DiagnosticVerdict
  /**
   * The aggregate refusal count for the current reporting window, or null while
   * the burst is still open. Resets after it returns a report, so the caller
   * writes at most one row per window.
   */
  takeRefusalReport(now: number): DiagnosticRefusalReport | null
  reset(): void
}

type SourceWindow = { start: number; count: number }

export function createDiagnosticThrottle(policy: DiagnosticPolicy): DiagnosticThrottle {
  const windows = new Map<string, SourceWindow>()
  let pending = 0
  let pendingReason: DiagnosticRefusalReason = 'rate'
  let reportWindowStart: number | null = null

  const refuse = (
    reason: DiagnosticRefusalReason,
    status: 429 | 413,
    now: number,
  ): DiagnosticVerdict => {
    pending += 1
    pendingReason = reason
    if (reportWindowStart === null) reportWindowStart = now
    return { allowed: false, reason, status }
  }

  return {
    checkDiagnosticWrite({ source, bodyBytes, now }) {
      const current = windows.get(source)
      const window =
        current && now - current.start < policy.windowMs
          ? current
          : { start: now, count: 0 }
      windows.set(source, window)

      if (bodyBytes > policy.maxBodyBytes) return refuse('size', 413, now)
      if (window.count >= policy.maxWrites) return refuse('rate', 429, now)
      window.count += 1
      return { allowed: true }
    },

    takeRefusalReport(now) {
      if (pending === 0) return null
      if (reportWindowStart === null) reportWindowStart = now
      if (now - reportWindowStart < policy.windowMs) return null
      const report = { count: pending, reason: pendingReason }
      pending = 0
      reportWindowStart = now
      return report
    },

    reset() {
      windows.clear()
      pending = 0
      reportWindowStart = null
    },
  }
}

/** A fixed, plain refusal. Nothing from the request is reflected into it. */
export function refusalResponse(status: 429 | 413): Response {
  return new Response(status === 429 ? 'Too Many Requests' : 'Payload Too Large', {
    status,
    headers: {
      'content-type': 'text/plain; charset=utf-8',
      'cache-control': 'no-store',
    },
  })
}

/**
 * The row an over-bound write leaves behind. It names the endpoint, the reason
 * and the aggregate count — and deliberately has no source field to fill.
 */
export function refusalRecord(input: {
  endpoint: string
  reason: DiagnosticRefusalReason
  count: number
}): DiagnosticRefusalRecord {
  return {
    kind: 'diagnostic-throttle',
    operation: `${input.endpoint}.throttled`,
    message: `diagnostic-throttled reason=${input.reason} n=${input.count}`,
    route: input.endpoint,
    level: 'warn',
  }
}

/**
 * Flush the aggregate refusal, if the window has closed, then answer plainly.
 * The caller injects its own recorder so this module never reaches for a store.
 */
export async function refuseDiagnosticWrite(input: {
  throttle: DiagnosticThrottle
  endpoint: string
  now: number
  status: 429 | 413
  record: (record: DiagnosticRefusalRecord) => Promise<unknown>
}): Promise<Response> {
  const report = input.throttle.takeRefusalReport(input.now)
  if (report) {
    await input
      .record(
        refusalRecord({
          endpoint: input.endpoint,
          reason: report.reason,
          count: report.count,
        }),
      )
      .catch(() => {
        /* a failure to record a refusal must not become a failure of the refusal */
      })
  }
  return refusalResponse(input.status)
}
