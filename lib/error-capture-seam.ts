// -----------------------------------------------------------------------------
// Capture seam helpers — low-friction wrappers so the sweep is mechanical.
//
//   withServerErrorCapture(label, asyncFn)         — wraps a server action /
//     async fn: on throw it captures durably then rethrows (caller decides).
//     SINGLE CALL: the curried form cannot infer the handler's argument types.
//   withApiHandler({ label, route }, handler)      — for Next route handlers:
//     on throw it captures and returns a 500 Response instead of letting a raw
//     error escape to a client.
//
// Errors are captured with severity + context and rethrown/handled — they never
// silently vanish, and never break the operation being recorded.
// -----------------------------------------------------------------------------
import { captureError, type ErrorLevel } from '@/legacy/db/app-error'

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

/**
 * Wrap an async server function: capture durably on throw, then rethrow.
 *
 * SINGLE CALL, deliberately. The curried form `withServerErrorCapture(label)(fn)`
 * cannot infer the handler's argument types — TypeScript pins TArgs to unknown[]
 * at the first call and the second call then fails to accept a typed handler.
 * `withApiHandler` was made single-call for exactly this reason (commit dca591b);
 * this now matches it.
 */
export function withServerErrorCapture<TArgs extends unknown[], TResult>(
  label: string,
  fn: (...args: TArgs) => Promise<TResult>,
  opts: CaptureOpts = {},
): (...args: TArgs) => Promise<TResult> {
  return async (...args: TArgs): Promise<TResult> => {
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
      // ALWAYS emit the primary failure before attempting durable capture. During an outage the
      // app_error database write can fail too; without this line the runtime log contains only the
      // secondary capture failure and hides the exception that actually broke the request.
      console.error('[api] request failed', {
        label: opts.label,
        route: opts.route ?? null,
        kind: err instanceof Error ? err.name || 'Error' : typeof err,
        message: messageOf(err, opts.label),
        ...(err && typeof err === 'object' && 'code' in err
          ? { code: String((err as { code?: unknown }).code ?? '') }
          : {}),
        ...(err && typeof err === 'object' && 'status' in err
          ? { status: Number((err as { status?: unknown }).status ?? 0) }
          : {}),
      })
      captureError({
        kind: err instanceof Error ? err.name || 'Error' : 'Error',
        operation: opts.label,
        message: messageOf(err, opts.label),
        stack: err instanceof Error ? err.stack ?? null : null,
        route: opts.route ?? null,
        level,
      })
      // A REFUSAL IS NOT AN INTERNAL ERROR.
      //
      // This answered `{ ok: false, error: 'internal_error' }` with a 500 for EVERY throw, including the ones a Service
      // deliberately raises to explain itself — "Under-contract and sold status are owned by the transaction workflow",
      // "Property status is invalid.", "Image is too large". The reason existed, was logged, and was thrown away on the
      // way to the screen: all the person clicking saw was `internal_error`, so they had to ask someone. That is the
      // single most expensive line in this file.
      //
      // Errors that describe themselves (a code, a message, a 4xx status) are now returned as they were raised. Only a
      // genuine crash — no code, no status, or a 5xx — stays a generic 500.
      const described = err as { code?: unknown; message?: unknown; status?: unknown } | null
      const code = described && typeof described.code === 'string' ? described.code : null
      const status =
        described && typeof described.status === 'number' && described.status >= 400 && described.status < 500
          ? described.status
          : null
      if (code && status !== null) {
        return new Response(
          JSON.stringify({
            ok: false,
            error: code,
            message: typeof described?.message === 'string' ? described.message : undefined,
          }),
          { status, headers: { 'content-type': 'application/json' } },
        )
      }
      return new Response(
        JSON.stringify({ ok: false, error: 'internal_error' }),
        { status: 500, headers: { 'content-type': 'application/json' } },
      )
    }
  }
}
