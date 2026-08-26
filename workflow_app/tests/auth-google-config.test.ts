import { test } from 'node:test'
import assert from 'node:assert/strict'

import { getAuthProviderConfig } from '../../lib/auth/provider-config'
import { resolveProviderSubject } from '../../db/auth-identity'

// ---------------------------------------------------------------------------
// AUTH-08 — Google OAuth configuration contract + canonical identity resolution.
// ---------------------------------------------------------------------------

function withEnv<V>(
  patch: Record<string, string | undefined>,
  fn: () => V,
): V {
  const saved: Record<string, string | undefined> = {}
  for (const [k, v] of Object.entries(patch)) {
    saved[k] = process.env[k]
    if (v === undefined) delete process.env[k]
    else process.env[k] = v
  }
  try {
    return fn()
  } finally {
    for (const k of Object.keys(patch)) {
      if (saved[k] === undefined) delete process.env[k]
      else process.env[k] = saved[k]
    }
  }
}

test('AUTH-08: Google is the default provider; unconfigured creds are null (fail closed)', () => {
  withEnv(
    { AUTH_PROVIDER: undefined, AUTH_GOOGLE_ID: undefined, AUTH_GOOGLE_SECRET: undefined },
    () => {
      const cfg = getAuthProviderConfig()
      assert.equal(cfg.provider, 'google')
      assert.equal(cfg.clientId, null)
      assert.equal(cfg.clientSecret, null)
    },
  )
})

test('AUTH-08: configured Google creds surface from env (names/values internal only)', () => {
  withEnv(
    {
      AUTH_PROVIDER: undefined,
      AUTH_GOOGLE_ID: 'g-client-id',
      AUTH_GOOGLE_SECRET: 'g-client-secret',
    },
    () => {
      const cfg = getAuthProviderConfig()
      assert.equal(cfg.provider, 'google')
      assert.equal(cfg.clientId, 'g-client-id')
      assert.equal(cfg.clientSecret, 'g-client-secret')
    },
  )
})

test('AUTH-08: an unmapped Google subject resolves as "unmapped" (no email guessing)', async () => {
  // Read-only canonical lookup; an obviously-fake subject maps to nothing.
  const result = await resolveProviderSubject(
    'google',
    'cl-integration-test-fake-subject-0000-0000',
  )
  assert.equal(result.kind, 'unmapped')
})
