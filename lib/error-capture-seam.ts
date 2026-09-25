import { recordRustAppDiagnostic } from '@/lib/rust-api/diagnostics'

export type ErrorLevel = 'info' | 'warn' | 'error' | 'fatal'

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

function capture(
  label: string,
  err: unknown,
  opts: CaptureOpts,
  level: ErrorLevel,
): void {
  const error = err instanceof Error ? err : new Error(String(err))
  void recordRustAppDiagnostic({
    kind: error.name || 'Error',
    operation: label,
    message: messageOf(err, label),
    route: opts.route ?? '',
    level,
    code:
      err && typeof err === 'object' && 'code' in err
        ? String((err as { code?: unknown }).code ?? '')
        : null,
    meta: {
      storyId: opts.storyId ?? null,
      stack: error.stack ?? null,
    },
  })
}

export function withServerErrorCapture<TArgs extends unknown[], TResult>(
  label: string,
  fn: (...args: TArgs) => Promise<TResult>,
  opts: CaptureOpts = {},
): (...args: TArgs) => Promise<TResult> {
  return async (...args: TArgs): Promise<TResult> => {
    try {
      return await fn(...args)
    } catch (err) {
      capture(label, err, opts, opts.level ?? levelOf(err))
      throw err
    }
  }
}

export function withApiHandler<A extends unknown[]>(
  opts: { label: string; route?: string | null },
  handler: (...args: A) => Promise<Response>,
): (...args: A) => Promise<Response> {
  return async (...args: A): Promise<Response> => {
    try {
      return await handler(...args)
    } catch (err) {
      const level = levelOf(err)
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
      capture(opts.label, err, { route: opts.route }, level)

      const described = err as
        | { code?: unknown; message?: unknown; status?: unknown }
        | null
      const code =
        described && typeof described.code === 'string' ? described.code : null
      const status =
        described &&
        typeof described.status === 'number' &&
        described.status >= 400 &&
        described.status < 500
          ? described.status
          : null
      if (code && status !== null) {
        return new Response(
          JSON.stringify({
            ok: false,
            error: code,
            message:
              typeof described?.message === 'string'
                ? described.message
                : undefined,
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
