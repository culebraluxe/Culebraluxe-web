import { createHash } from 'node:crypto'

export type RustBridgeIdentity = {
  provider: string
  providerSubject: string
}

export function resolveRustApiBaseUrl(
  configured: string | undefined,
  nodeEnv: string | undefined,
): string | null {
  const value = configured?.trim()
  if (value) return value.replace(/\/+$/, '')
  return nodeEnv === 'production' ? null : 'http://127.0.0.1:8080'
}

export function resolveInternalApiKey(
  value: string | undefined,
  authSecret?: string,
): string | null {
  const key = value?.trim()
  if (key && key.length >= 16) return key

  const secret = authSecret?.trim()
  if (!secret || secret.length < 16) return null

  return createHash('sha256')
    .update('culebraluxe-rust-bridge:v1:')
    .update(secret)
    .digest('hex')
}

// A plain record is intentional: JSON bridge writers add content-type with
// object spread. Returning Headers here would silently discard every bridge
// header because Headers entries are not enumerable object properties.
export type RustBridgeHeaders = Record<string, string>

export function buildRustBridgeHeaders(input: {
  identity: RustBridgeIdentity
  internalApiKey: string
  correlationId: string
  causationId?: string
}): RustBridgeHeaders {
  const headers: RustBridgeHeaders = {
    accept: 'application/json',
    'x-culebra-internal-key': input.internalApiKey,
    'x-culebra-auth-provider': input.identity.provider,
    'x-culebra-auth-sub': input.identity.providerSubject,
    'x-culebra-correlation-id': input.correlationId,
  }

  const causationId = input.causationId?.trim()
  if (causationId) headers['x-culebra-causation-id'] = causationId

  return headers
}