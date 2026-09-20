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

export function resolveInternalApiKey(value: string | undefined): string | null {
  const key = value?.trim()
  return key && key.length >= 16 ? key : null
}

export function buildRustBridgeHeaders(input: {
  identity: RustBridgeIdentity
  internalApiKey: string
  correlationId: string
  causationId?: string
}): Headers {
  const headers = new Headers({
    accept: 'application/json',
    'x-culebra-internal-key': input.internalApiKey,
    'x-culebra-auth-provider': input.identity.provider,
    'x-culebra-auth-sub': input.identity.providerSubject,
    'x-culebra-correlation-id': input.correlationId,
  })

  const causationId = input.causationId?.trim()
  if (causationId) headers.set('x-culebra-causation-id', causationId)

  return headers
}
