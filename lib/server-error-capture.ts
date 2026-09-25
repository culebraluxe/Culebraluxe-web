import { recordRustAppDiagnostic } from '@/lib/rust-api/diagnostics'

export type ErrorLevel = 'info' | 'warn' | 'error' | 'fatal'

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

export function captureServerError(
  label: string,
  err: unknown,
  opts: CaptureOpts = {},
): Error {
  const level = opts.level ?? defaultLevel(err)
  const error = err instanceof Error ? err : new Error(String(err))
  const message = error.message.trim() || label
  console.error(`${LEVEL_PREFIX[level]} ${label}: ${message}`)
  void recordRustAppDiagnostic({
    kind: error.name || 'Error',
    operation: label,
    message,
    route: opts.route ?? '',
    level,
    meta: {
      storyId: opts.storyId ?? null,
      stack: error.stack ?? null,
    },
  })
  return error
}

export function captureServerLog(
  level: 'info' | 'warn',
  label: string,
  message: string,
  opts: CaptureOpts = {},
): void {
  const text = message.trim() || label
  const marker = level === 'info' ? LEVEL_PREFIX.info : LEVEL_PREFIX.warn
  console.log(`${marker} ${label}: ${text}`)
  void recordRustAppDiagnostic({
    kind: level === 'info' ? 'INFO' : 'WARN',
    operation: label,
    message: text,
    route: opts.route ?? '',
    level,
    meta: { storyId: opts.storyId ?? null },
  })
}
