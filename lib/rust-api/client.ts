import 'server-only'

import { randomUUID } from 'node:crypto'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { bypassBridgeIdentity } from '@/lib/rust-api/dev-identity'
import {
  buildRustBridgeHeaders,
  resolveInternalApiKey,
  resolveRustApiBaseUrl,
} from '@/lib/rust-api/contract'

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
  const value = resolveRustApiBaseUrl(process.env.RUST_API_BASE_URL, process.env.NODE_ENV)
  if (value) return value
  throw new RustApiError({
    status: 503,
    code: 'RUST_API_UNAVAILABLE',
    message: 'RUST_API_BASE_URL is not configured.',
    retryable: true,
  })
}

function internalApiKey(): string {
  const value = resolveInternalApiKey(
    process.env.CULEBRA_INTERNAL_API_KEY,
    process.env.AUTH_SECRET,
  )
  if (value) return value
  throw new RustApiError({
    status: 503,
    code: 'RUST_API_UNAVAILABLE',
    message: 'CULEBRA_INTERNAL_API_KEY is not configured.',
    retryable: false,
  })
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
  const identity =
    (await createAuthJsSessionAdapter().getSession()) ??
    // A bypassed dev session has no provider identity; this presents the real auth_identity for the bypass user so the
    // Rust API resolves the same application user it would for a signed-in caller. Returns null in production, by its
    // own guard, so this line cannot weaken anything that ships.
    (await bypassBridgeIdentity())
  if (!identity) {
    throw new RustApiError({
      status: 401,
      code: 'AUTH_IDENTITY_REQUIRED',
      message: 'An authenticated provider identity is required.',
    })
  }

  const correlationId = options.correlationId?.trim() || randomUUID()
  const headers = buildRustBridgeHeaders({
    identity,
    internalApiKey: internalApiKey(),
    correlationId,
    causationId: options.causationId,
  })

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


/**
 * JSON command bridge, restricted by its own type to the reviewed engine routes.
 *
 * The earlier rule here was "no generic write method", and the reason mattered: an open door to arbitrary writes would
 * let any caller cut a route over before anyone had reviewed it. That reason is preserved rather than dropped - the
 * path type below admits only `/v1/engine/...`, so this is not a general write door, it is the engine's door. Adding a
 * new engine command means adding a route on the Rust side and naming it in the union here, which is a review.
 *
 * Identity resolves exactly as it does for reads, bypass bridge included, so a dev session still attributes the command
 * to the real user the Rust side resolves.
 */
export type RustApiEnginePath =
  | '/v1/engine/transactions'
  | '/v1/engine/timers/reconcile'
  | '/v1/engine/tasks/complete'
  | '/v1/engine/reclaim'

export async function rustApiEngineCommand<T>(
  path: RustApiEnginePath,
  body: Record<string, unknown>,
  options: RustApiReadOptions = {},
): Promise<RustApiSuccess<T>> {
  const identity =
    (await createAuthJsSessionAdapter().getSession()) ?? (await bypassBridgeIdentity())
  if (!identity) {
    throw new RustApiError({
      status: 401,
      code: 'AUTH_IDENTITY_REQUIRED',
      message: 'An authenticated provider identity is required.',
    })
  }

  const correlationId = options.correlationId?.trim() || randomUUID()
  const headers = {
    ...buildRustBridgeHeaders({
      identity,
      internalApiKey: internalApiKey(),
      correlationId,
      causationId: options.causationId,
    }),
    'content-type': 'application/json',
  }

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}${path}`, {
      method: 'POST',
      headers,
      body: JSON.stringify(body),
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


/**
 * Multipart write bridge for commands that are already authenticated and
 * validated at the Next edge. Rust re-resolves the canonical user and owns the
 * business transaction.
 */
export async function rustApiWriteForm<T>(
  path: `/v1/${string}`,
  formData: FormData,
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
  const headers = buildRustBridgeHeaders({
    identity,
    internalApiKey: internalApiKey(),
    correlationId,
    causationId: options.causationId,
  })

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}${path}`, {
      method: 'POST',
      headers,
      body: formData,
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


type RustApiJsonWriteOptions = RustApiReadOptions

async function rustApiJsonWrite<T>(
  path: `/v1/${string}`,
  method: 'POST' | 'PATCH',
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  const identity =
    (await createAuthJsSessionAdapter().getSession()) ??
    (await bypassBridgeIdentity())
  if (!identity) {
    throw new RustApiError({
      status: 401,
      code: 'AUTH_IDENTITY_REQUIRED',
      message: 'An authenticated provider identity is required.',
    })
  }

  const correlationId = options.correlationId?.trim() || randomUUID()
  const headers = {
    ...buildRustBridgeHeaders({
      identity,
      internalApiKey: internalApiKey(),
      correlationId,
      causationId: options.causationId,
    }),
    'content-type': 'application/json',
  }

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}${path}`, {
      method,
      headers,
      body: JSON.stringify(body),
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

export async function rustApiCreateForm<T>(
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>('/v1/forms', 'POST', body, options)
}

export async function rustApiUpdateForm<T>(
  formId: string,
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/forms/${encodeURIComponent(formId)}`) as `/v1/${string}`,
    'PATCH',
    body,
    options,
  )
}


export async function rustApiUpdateProject<T>(
  projectId: string,
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/projects/${encodeURIComponent(projectId)}`) as `/v1/${string}`,
    'PATCH',
    body,
    options,
  )
}

export async function rustApiUpdateWbs<T>(
  itemId: string,
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/wbs/${encodeURIComponent(itemId)}`) as `/v1/${string}`,
    'PATCH',
    body,
    options,
  )
}
