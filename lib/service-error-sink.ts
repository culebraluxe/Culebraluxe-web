import { recordRustAppDiagnostic } from '@/lib/rust-api/diagnostics'

export type ServiceFailureRecord = {
  code: string
  domain: string
  operation: string
  message: string
  stack?: string | null
}

export type ServiceErrorSink = {
  record(failure: ServiceFailureRecord): Promise<void>
}

export function createNeonServiceErrorSink(): ServiceErrorSink {
  return {
    record: async (failure) => {
      await recordRustAppDiagnostic({
        kind: `service:${failure.code}`,
        operation: `service:${failure.domain}:${failure.operation}`,
        message: failure.message,
        route: '',
        level: 'error',
        code: failure.code,
        meta: { stack: failure.stack ?? null },
      })
    },
  }
}

let sharedSink: ServiceErrorSink | null = null

export function appServiceErrorSink(): ServiceErrorSink {
  if (!sharedSink) sharedSink = createNeonServiceErrorSink()
  return sharedSink
}
