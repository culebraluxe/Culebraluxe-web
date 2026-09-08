// -----------------------------------------------------------------------------
// Capture seam helpers — low-friction wrappers so the sweep is mechanical.
//
//   withServerErrorCapture(label, opts)(asyncFn)   — wraps a server action /
//     async fn: on throw it captures durably then rethrows (caller decides).
//   withApiHandler(label, opts)(handler)            — for Next route handlers:
//     on throw it captures and returns a 500 Response instead of letting a raw
//     error escape to a client.
//
// Errors are captured with severity + context and rethrown/handled — they never
// silently vanish, and never break the operation being recorded.
// -----------------------------------------------------------------------------
import { captureError, type ErrorLevel } from '../db/app-error'

export type CaptureOpts = {
  level?: ErrorLevel
  storyId?: string | null
  route?: string | null
}

function levelOf(err: unknown): ErrorLevel {
  const name = err instanceof Error ? err.name : typeof err
  if (/fatal/i.test(name)) return 'fatal'
  if (err instanceof Error && /warn/i.test(name)) return 'warn'
  return 'error'
}

function messageOf(err: unknown, label: string): string {
  if (err instanceof Error) return err.message.trim() || label
  return String(err)
}

/** Wrap an async server function: capture durably on throw, then rethrow. */
export function withServerErrorCapture<TArgs extends unknown[], TResult>(
  label: string,
  opts: CaptureOpts = {},
) {
  return (fn: (...args: TArgs) => Promise<TResult>) =>
    async (...args: TArgs): Promise<TResult> => {
      try {
        return await fn(...args)
      } catch (err) {
        const level = opts.level ?? levelOf(err)
        captureError({
          kind: err instanceof Error ? err.name || 'Error' : 'Error',
          operation: label,
          message: messageOf(err, label),
          stack: err instanceof Error ? err.stack ?? null : null,
          storyId: opts.storyId ?? null,
          route: opts.route ?? null,
          level,
        })
        throw err
      }
    }
}

/** Wrap a Next route handler: capture durably on throw, return a 500 Response. */
export function withApiHandler<A extends unknown[]>(
  opts: { label: string; route?: string | null },
  handler: (...args: A) => Promise<Response>,
): (...args: A) => Promise<Response> {
  return async (...args: A): Promise<Response> => {
    try {
      return await handler(...args)
    } catch (err) {
      const level = levelOf(err)
      captureError({
        kind: err instanceof Error ? err.name || 'Error' : 'Error',
        operation: opts.label,
        message: messageOf(err, opts.label),
        stack: err instanceof Error ? err.stack ?? null : null,
        route: opts.route ?? null,
        level,
      })
      return new Response(
        JSON.stringify({ ok: false, error: 'internal_error' }),
        { status: 500, headers: { 'content-type': 'application/json' } },
      )
    }
  }
}
