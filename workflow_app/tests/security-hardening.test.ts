import { test } from 'node:test'
import assert from 'node:assert/strict'

import { isPortalAuthBypass } from '../../lib/auth/dev-bypass'
import { getBreakGlassConfig } from '../../lib/auth/break-glass-config'
import {
  hashBreakGlassSecret,
  verifyBreakGlassSecret,
} from '../../lib/auth/break-glass-secret'
import {
  MAX_MEDIA_UPLOAD_BYTES,
  sanitizeUploadFilename,
  validateMediaUpload,
} from '../../lib/media/upload-policy'

// ---------------------------------------------------------------------------
// SECURITY HARDENING (HARDEN-01/04/06, break-glass) focused proofs.
// ---------------------------------------------------------------------------

/** Run fn with a patched process.env, restoring prior values afterwards. */
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

// --- HARDEN-01 / HARDEN-04 — dev bypass must fail closed in production ---
test('HARDEN-01/04: DEV bypass is never honored in production', () => {
  withEnv(
    {
      NODE_TEST_CONTEXT: undefined,
      PORTAL_AUTH_BYPASS: '1',
      NODE_ENV: 'production',
    },
    () => assert.equal(isPortalAuthBypass(), false),
  )
})

test('HARDEN-01/04: DEV bypass still works in development', () => {
  withEnv(
    {
      NODE_TEST_CONTEXT: undefined,
      PORTAL_AUTH_BYPASS: '1',
      NODE_ENV: 'development',
    },
    () => assert.equal(isPortalAuthBypass(), true),
  )
})

test('HARDEN-01: no flag means no bypass (fail closed)', () => {
  withEnv(
    { NODE_TEST_CONTEXT: undefined, PORTAL_AUTH_BYPASS: undefined, NODE_ENV: 'development' },
    () => assert.equal(isPortalAuthBypass(), false),
  )
})

// --- break-glass secret ---
test('break-glass: correct secret verifies, wrong secret fails (timing-safe)', () => {
  const hash = hashBreakGlassSecret('test-bootstrap-password')
  assert.equal(verifyBreakGlassSecret('test-bootstrap-password', hash), true)
  assert.equal(verifyBreakGlassSecret('wrong-password', hash), false)
  assert.equal(verifyBreakGlassSecret('', hash), false)
})

test('break-glass: config is env-backed and disabled when unconfigured', () => {
  withEnv(
    { AUTH_BREAK_GLASS_ENABLED: undefined, AUTH_BREAK_GLASS_SECRET_HASH: undefined, AUTH_BREAK_GLASS_APP_USER_ID: undefined },
    () => {
      const cfg = getBreakGlassConfig()
      assert.equal(cfg.enabled, false)
      assert.equal(cfg.secretHash, null)
      assert.equal(cfg.appUserId, null)
    },
  )
  withEnv(
    {
      AUTH_BREAK_GLASS_ENABLED: 'true',
      AUTH_BREAK_GLASS_SECRET_HASH: 'scrypt$aa$bb',
      AUTH_BREAK_GLASS_APP_USER_ID: 'u-owner',
    },
    () => {
      const cfg = getBreakGlassConfig()
      assert.equal(cfg.enabled, true)
      assert.equal(cfg.secretHash, 'scrypt$aa$bb')
      assert.equal(cfg.appUserId, 'u-owner')
    },
  )
})

// --- HARDEN-06 — upload policy ---
test('HARDEN-06: valid image/pdf uploads are accepted', () => {
  assert.deepEqual(
    validateMediaUpload({ size: 1024, type: 'image/jpeg', name: 'a.jpg' }),
    { ok: true },
  )
  assert.deepEqual(
    validateMediaUpload({ size: 1024, type: 'application/pdf', name: 'a.pdf' }),
    { ok: true },
  )
})

test('HARDEN-06: empty, oversized, and disallowed MIME uploads are rejected', () => {
  assert.equal(validateMediaUpload({ size: 0, type: 'image/jpeg', name: 'a.jpg' }).ok, false)
  assert.equal(
    validateMediaUpload({ size: MAX_MEDIA_UPLOAD_BYTES + 1, type: 'image/jpeg', name: 'a.jpg' }).ok,
    false,
  )
  assert.equal(
    validateMediaUpload({ size: 1024, type: 'application/x-msdownload', name: 'a.exe' }).ok,
    false,
  )
})

test('HARDEN-06: filenames are sanitized (no path separators / control chars)', () => {
  assert.equal(sanitizeUploadFilename('../../etc/passwd'), 'passwd')
  assert.equal(sanitizeUploadFilename('..\\..\\windows\\evil.exe'), 'evil.exe')
  assert.equal(sanitizeUploadFilename('photo 1.jpg'), 'photo 1.jpg')
  assert.equal(sanitizeUploadFilename(''), 'upload')
})
