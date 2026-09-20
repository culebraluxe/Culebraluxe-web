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
  assert.equal(resolveInternalApiKey(undefined), null)
  assert.equal(resolveInternalApiKey(' short '), null)
  assert.equal(resolveInternalApiKey(' 1234567890abcdef '), '1234567890abcdef')
})

test('bridge forwards provider identity but no role or security level', () => {
  const headers = buildRustBridgeHeaders({
    identity: {
      provider: 'google',
      providerSubject: 'stable-provider-subject',
    },
    internalApiKey: '1234567890abcdef',
    correlationId: 'corr-123',
    causationId: ' cause-456 ',
  })

  assert.equal(headers.get('x-culebra-auth-provider'), 'google')
  assert.equal(headers.get('x-culebra-auth-sub'), 'stable-provider-subject')
  assert.equal(headers.get('x-culebra-correlation-id'), 'corr-123')
  assert.equal(headers.get('x-culebra-causation-id'), 'cause-456')
  assert.equal(headers.get('x-culebra-internal-key'), '1234567890abcdef')
  assert.equal(headers.get('x-culebra-security-level'), null)
  assert.equal(headers.get('x-culebra-role-codes'), null)
})
