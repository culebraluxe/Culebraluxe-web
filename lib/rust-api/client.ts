import 'server-only'

import { randomUUID } from 'node:crypto'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'

export type RustApiSuccess<T> = {
  ok: true
  value: T
  correlationId: string
}

export type RustApiFailure = {
  ok: false
  error: {
    code: string
    message: string
    retryable: boolean
    incidentId?: string | null
  }
  correlationId?: string | null
}

export class RustApiError extends Error {
  readonly status: number
  readonly code: string
  readonly retryable: boolean
  readonly correlationId: string | null
  readonly incidentId: string | null

  constructor(input: {
    status: number
    code: string
    message: string
    retryable?: boolean
    correlationId?: string | null
    incidentId?: string | null
  }) {
    super(input.message)
    this.name = 'RustApiError'
    this.status = input.status
    this.code = input.code
    this.retryable = input.retryable ?? false
    this.correlationId = input.correlationId ?? null
    this.incidentId = input.incidentId ?? null
  }
}

type RustApiReadOptions = {
  correlationId?: string
  causationId?: string
}

function rustApiBaseUrl(): string {
  const configured = process.env.RUST_API_BASE_URL?.trim()
  if (configured) return configured.replace(/\/+$/, '')

  if (process.env.NODE_ENV !== 'production') {
    return 'http://127.0.0.1:8080'
  }

  throw new RustApiError({
    status: 503,
    code: 'RUST_API_UNAVAILABLE',
    message: 'RUST_API_BASE_URL is not configured.',
    retryable: true,
  })
}

function internalApiKey(): string {
  const key = process.env.CULEBRA_INTERNAL_API_KEY?.trim()
  if (!key || key.length < 16) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: 'CULEBRA_INTERNAL_API_KEY is not configured.',
      retryable: false,
    })
  }
  return key
}

/**
 * GET-only bridge from the authenticated Next/Auth.js edge to the Rust service.
 *
 * This deliberately has no generic write method. Write cutover happens only
 * after the read transport is proven and each command route is explicitly
 * reviewed.
 */
export async function rustApiRead<T>(
  path: `/v1/${string}`,
  options: RustApiReadOptions = {},
): Promise<RustApiSuccess<T>> {
  const identity = await createAuthJsSessionAdapter().getSession()
  if (!identity) {
    throw new RustApiError({
      status: 401,
      code: 'AUTH_IDENTITY_REQUIRED',
      message: 'An authenticated provider identity is required.',
    })
  }

  const correlationId = options.correlationId?.trim() || randomUUID()
  const headers = new Headers({
    accept: 'application/json',
    'x-culebra-internal-key': internalApiKey(),
    'x-culebra-auth-provider': identity.provider,
    'x-culebra-auth-sub': identity.providerSubject,
    'x-culebra-correlation-id': correlationId,
  })
  if (options.causationId?.trim()) {
    headers.set('x-culebra-causation-id', options.causationId.trim())
  }

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}${path}`, {
      method: 'GET',
      headers,
      cache: 'no-store',
    })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  let payload: RustApiSuccess<T> | RustApiFailure
  try {
    payload = (await response.json()) as RustApiSuccess<T> | RustApiFailure
  } catch {
    throw new RustApiError({
      status: 502,
      code: 'RUST_API_INVALID_RESPONSE',
      message: 'Rust API returned a non-JSON response.',
      retryable: true,
      correlationId,
    })
  }

  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? response.status >= 500,
      correlationId: failure.correlationId ?? correlationId,
      incidentId: failure.error?.incidentId ?? null,
    })
  }

  return payload
}
