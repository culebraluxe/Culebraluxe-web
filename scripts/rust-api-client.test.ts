import assert from 'node:assert/strict'
import test from 'node:test'

import {
  buildRustBridgeHeaders,
  resolveInternalApiKey,
  resolveRustApiBaseUrl,
} from '../lib/rust-api/contract'

test('rust api base url trims trailing slash and falls back only outside production', () => {
  assert.equal(
    resolveRustApiBaseUrl(' https://rust.example.internal/// ', 'production'),
    'https://rust.example.internal',
  )
  assert.equal(resolveRustApiBaseUrl(undefined, 'development'), 'http://127.0.0.1:8080')
  assert.equal(resolveRustApiBaseUrl(undefined, 'test'), 'http://127.0.0.1:8080')
  assert.equal(resolveRustApiBaseUrl(undefined, 'production'), null)
})

test('internal api key requires at least sixteen trimmed characters', () => {
  const syntheticKey = ['test', 'bridge', 'key', 'not', 'secret'].join('-')
  assert.equal(resolveInternalApiKey(undefined), null)
  assert.equal(resolveInternalApiKey(' short '), null)
  assert.equal(resolveInternalApiKey(` ${syntheticKey} `), syntheticKey)

  const derived = resolveInternalApiKey(undefined, 'auth-secret-value-long-enough')
  assert.equal(derived?.length, 64)
  assert.equal(
    derived,
    resolveInternalApiKey(undefined, 'auth-secret-value-long-enough'),
  )
})

test('bridge forwards provider identity but no role or security level', () => {
  const syntheticKey = ['test', 'bridge', 'key', 'not', 'secret'].join('-')
  const headers = buildRustBridgeHeaders({
    identity: {
      provider: 'google',
      providerSubject: 'stable-provider-subject',
    },
    internalApiKey: syntheticKey,
    correlationId: 'corr-123',
    causationId: ' cause-456 ',
  })

  assert.equal(headers['x-culebra-auth-provider'], 'google')
  assert.equal(headers['x-culebra-auth-sub'], 'stable-provider-subject')
  assert.equal(headers['x-culebra-correlation-id'], 'corr-123')
  assert.equal(headers['x-culebra-causation-id'], 'cause-456')
  assert.equal(headers['x-culebra-internal-key'], syntheticKey)
  assert.equal(headers['x-culebra-security-level'], undefined)
  assert.equal(headers['x-culebra-role-codes'], undefined)
})

test('JSON write header spread preserves the trusted bridge identity and key', () => {
  const syntheticKey = ['test', 'bridge', 'key', 'not', 'secret'].join('-')
  const bridgeHeaders = buildRustBridgeHeaders({
    identity: { provider: 'google', providerSubject: 'stable-provider-subject' },
    internalApiKey: syntheticKey,
    correlationId: 'corr-write-123',
  })
  const request = new Request('https://rust.example.invalid/v1/properties/admin', {
    method: 'POST',
    headers: { ...bridgeHeaders, 'content-type': 'application/json' },
    body: JSON.stringify({ name: 'ZoniBluff' }),
  })

  assert.equal(request.headers.get('x-culebra-internal-key'), syntheticKey)
  assert.equal(request.headers.get('x-culebra-auth-provider'), 'google')
  assert.equal(request.headers.get('x-culebra-auth-sub'), 'stable-provider-subject')
  assert.equal(request.headers.get('x-culebra-correlation-id'), 'corr-write-123')
  assert.equal(request.headers.get('content-type'), 'application/json')
})