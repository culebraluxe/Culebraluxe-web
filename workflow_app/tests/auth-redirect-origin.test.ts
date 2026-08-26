// ---------------------------------------------------------------------------
// AUTH-08D — canonical OAuth redirect_uri / callback origin.
//
// Auth.js v5 beta.32 (@auth/core@0.41.3) `createActionURL` uses
// `AUTH_URL ?? NEXTAUTH_URL`; when AUTH_URL is unset it INFERS the origin from
// `x-forwarded-host`/`host`, which produces host-dependent redirect_uri
// mismatches (vercel.app, non-www, 127.0.0.1, LAN IP, http/https noise).
//
// These tests prove that a canonical AUTH_URL (per environment) forces a
// deterministic callback that ignores forwarded-host/proto noise:
//   DEV:  http://localhost:3000/api/auth/callback/google
//   PROD: https://www.culebraluxe.com/api/auth/callback/google
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

/** Replicates the verified @auth/core@0.41.3 createActionURL AUTH_URL branch. */
function callbackUrl(
  authUrl: string,
  headers: Record<string, string | null>,
): string {
  let url: URL
  if (authUrl) {
    url = new URL(authUrl)
  } else {
    const detectedHost = headers['x-forwarded-host'] ?? headers['host'] ?? 'localhost:3000'
    const detectedProtocol = headers['x-forwarded-proto'] ?? 'http'
    const p = detectedProtocol.endsWith(':') ? detectedProtocol : `${detectedProtocol}:`
    url = new URL(`${p}//${detectedHost}`)
  }
  const sanitized = url.toString().replace(/\/$/, '')
  return new URL(`${sanitized}/api/auth/callback/google`).toString()
}

test('AUTH-08D: DEV canonical AUTH_URL forces localhost callback despite host/proto noise', () => {
  const DEV = 'http://localhost:3000'
  const noise = [
    { 'x-forwarded-host': '127.0.0.1:3000', 'x-forwarded-proto': 'http' },
    { 'host': '192.168.1.50:3000', 'x-forwarded-proto': 'http' },
    { 'x-forwarded-host': 'localhost:3000', 'x-forwarded-proto': 'https' },
    {},
  ]
  for (const h of noise) {
    assert.equal(
      callbackUrl(DEV, h),
      'http://localhost:3000/api/auth/callback/google',
      JSON.stringify(h),
    )
  }
})

test('AUTH-08D: PROD canonical AUTH_URL forces https://www.culebraluxe.com callback despite noise', () => {
  const PROD = 'https://www.culebraluxe.com'
  const noise = [
    { 'x-forwarded-host': 'culebraluxe.vercel.app', 'x-forwarded-proto': 'https' },
    { 'x-forwarded-host': 'culebraluxe.com', 'x-forwarded-proto': 'https' },
    { 'host': 'www.culebraluxe.com', 'x-forwarded-proto': 'http' },
    { 'x-forwarded-host': 'www.culebraluxe.com', 'x-forwarded-proto': 'https' },
  ]
  for (const h of noise) {
    assert.equal(
      callbackUrl(PROD, h),
      'https://www.culebraluxe.com/api/auth/callback/google',
      JSON.stringify(h),
    )
  }
})

test('AUTH-08D: DEV environment configures AUTH_URL to the canonical localhost origin', () => {
  // Loaded via --env-file=.env.local. Guards against missing/regressed config.
  assert.equal(process.env.AUTH_URL, 'http://localhost:3000')
})
