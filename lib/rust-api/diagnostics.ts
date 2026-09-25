import 'server-only'

import { randomUUID } from 'node:crypto'

import {
  buildRustPublicBridgeHeaders,
  resolveInternalApiKey,
  resolveRustApiBaseUrl,
} from '@/lib/rust-api/contract'

export type AppDiagnosticEvent = {
  kind: string
  operation: string
  message: string
  route: string
  level: string
  code?: string | null
  meta?: Record<string, unknown>
}

export async function recordRustAppDiagnostic(
  event: AppDiagnosticEvent,
): Promise<void> {
  const base = resolveRustApiBaseUrl(
    process.env.RUST_API_BASE_URL,
    process.env.NODE_ENV,
  )
  const key = resolveInternalApiKey(
    process.env.CULEBRA_INTERNAL_API_KEY,
    process.env.AUTH_SECRET,
  )
  if (!base || !key) return

  const correlationId = randomUUID()
  const response = await fetch(`${base}/v1/diagnostics/app-error`, {
    method: 'POST',
    headers: {
      ...buildRustPublicBridgeHeaders({
        internalApiKey: key,
        correlationId,
      }),
      'content-type': 'application/json',
    },
    body: JSON.stringify(event),
    cache: 'no-store',
  }).catch(() => null)

  // Diagnostic capture is best effort by design.
  if (!response?.ok) return
}
