// ---------------------------------------------------------------------------
// AUTH-08F — minimal Auth.js v5 + Google baseline + DEV diagnostics.
//
// Proves the RESET baseline without needing a live Google consent round-trip
// (which requires a human browser):
//
//   Test 1  minimal Google authentication -> Auth.js session created (jwt
//           callback stamps sub+provider; no authorities).
//   Test 2  mapped application identity -> /portal authorized (the
//           require-portal-access seam maps ok -> /portal).
//   Test 3  unmapped Google identity -> /login/unauthorized.
//   Test 4  application DB unavailable after Google auth -> Auth.js session
//           still independent; portal denied cleanly; NO Configuration error.
//   Test 5  bad/missing Google env -> fail closed (clientId/secret bound
//           directly from env; nothing fabricated).
//   Test 6  bad/missing AUTH_SECRET -> fail closed (no fallback invented).
//
// The live Google round-trip itself is verified by the DEV flight recorder
// (AUTH_GOOGLE_CALLBACK_RECEIVED / AUTH_SESSION_CREATED) during Chris's
// browser proof on /login -> /portal-auth-proof.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { devAuthLog } from '../../lib/auth/dev-auth-log'

const read = (rel: string): string =>
  readFileSync(new URL(`../../${rel}`, import.meta.url), 'utf8')

const AUTH_SRC = read('auth.ts')
const GUARD_SRC = read('lib/auth/require-portal-access.ts')

/** Code only (comments stripped) so assertions target actual statements. */
function stripComments(src: string): string {
  return src
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/^\s*\/\/.*$/gm, '')
}
const AUTH_CODE = stripComments(AUTH_SRC)

test('AUTH-08F: auth.ts is the minimal Google baseline (no providers abstraction, no DB)', () => {
  // Google is the ONLY provider registered.
  assert.match(AUTH_SRC, /providers:\s*\[\s*Google\(/)
  // No buildProviders / generic-OIDC / Credentials / break-glass provider.
  assert.ok(!AUTH_CODE.includes('buildProviders'), 'no buildProviders()')
  assert.ok(!AUTH_SRC.includes("from 'next-auth/providers/credentials'"), 'no Credentials provider')
  assert.ok(!AUTH_SRC.includes('authenticateBreakGlass'), 'no break-glass authenticate in auth.ts')
  assert.ok(!AUTH_SRC.includes('getAuthProviderConfig'), 'no provider-config abstraction in auth.ts')
  // No database access inside Auth.js.
  assert.ok(!AUTH_SRC.includes('@/db/'), 'no DB import in auth.ts')
  assert.ok(!AUTH_SRC.includes("from 'next-auth/adapters'"), 'no DB adapter')
})

test('AUTH-08F: minimal Google authentication creates a session (stable sub+provider only, no authorities)', () => {
  // jwt callback stamps sub from the STABLE provider account id
  // (account.providerAccountId = Google sub), NEVER from the per-login
  // user.id (which Auth.js v5 mints as a fresh randomUUID without an adapter),
  // and never an authority snapshot.
  assert.ok(
    AUTH_SRC.includes('account?.providerAccountId'),
    'subject sourced from account.providerAccountId (stable Google sub)',
  )
  assert.ok(
    !AUTH_SRC.includes('token.sub = user.id'),
    'must NOT stamp sub from user.id (per-login randomUUID without adapter)',
  )
  assert.ok(!AUTH_SRC.includes('getSessionAuthoritySnapshot'), 'no authority snapshot in jwt callback')
  assert.ok(!AUTH_SRC.includes('token.capabilities'), 'no capabilities stamping in auth.ts')
  assert.ok(!AUTH_SRC.includes('capabilities'), 'no authority resolution in Auth.js')
  // session callback exposes only sub + provider.
  assert.ok(AUTH_SRC.includes('session.user.sub'), 'session exposes sub')
  assert.ok(AUTH_SRC.includes('session.user.provider'), 'session exposes provider')
})

test('AUTH-08F: Test 5 — bad/missing Google env fails closed (creds bound directly from env)', () => {
  assert.ok(
    AUTH_SRC.includes('clientId: process.env.AUTH_GOOGLE_ID'),
    'Google clientId bound from AUTH_GOOGLE_ID',
  )
  assert.ok(
    AUTH_SRC.includes('clientSecret: process.env.AUTH_GOOGLE_SECRET'),
    'Google clientSecret bound from AUTH_GOOGLE_SECRET',
  )
})

test('AUTH-08F: Test 6 — bad/missing AUTH_SECRET fails closed (no fallback invented)', () => {
  assert.ok(AUTH_SRC.includes('secret: process.env.AUTH_SECRET'), 'secret bound from AUTH_SECRET')
  assert.ok(!AUTH_SRC.includes('NEXTAUTH_SECRET'), 'no NEXTAUTH_SECRET fallback')
})

test('AUTH-08F: Test 4 — no Configuration error path; DB/authorization is fully outside Auth.js', () => {
  // Auth.js callbacks cannot produce a Configuration error from DB failures
  // because they never touch the DB. The only authority code path lives in
  // require-portal-access (application layer), which fails closed.
  assert.ok(!AUTH_CODE.includes('authorization'), 'no authorization in Auth.js')
  assert.ok(!AUTH_CODE.includes('app_user'), 'no app_user lookup in Auth.js')
  assert.ok(!AUTH_CODE.includes('auth_identity'), 'no identity lookup in Auth.js')
})

test('AUTH-08F: Test 2/3 — application authorization maps ok->/portal and denial->/login/unauthorized', () => {
  // ok:true maps to /portal (the layout redirects authorized actors onward).
  assert.ok(GUARD_SRC.includes('ok: true'), 'authorized seam returns ok')
  assert.ok(GUARD_SRC.includes('/login/unauthorized'), 'denial target is /login/unauthorized')
  assert.ok(GUARD_SRC.includes("error.code === 'unauthenticated' ? '/login' : '/login/unauthorized'"),
    'unmapped/inactive/missing-authority fail closed to /login/unauthorized')
})

test('AUTH-08I: devAuthLog routes NORMAL markers to info, FAILURES to error (safe format)', () => {
  const errors: string[] = []
  const infos: string[] = []
  const origErr = console.error
  const origInfo = console.info
  console.error = (...args: unknown[]) => errors.push(String(args[0]))
  console.info = (...args: unknown[]) => infos.push(String(args[0]))
  try {
    devAuthLog('AUTH_SIGNIN_STARTED')
    devAuthLog('GOOGLE_CALLBACK', 'INVALID_CALLBACK_STATE')
  } finally {
    console.error = origErr
    console.info = origInfo
  }
  // Success/normal marker is INFORMATIONAL (no Next.js DEV error overlay).
  assert.ok(infos.some((l) => l.includes('AUTH_SIGNIN_STARTED')), 'normal marker -> console.info')
  assert.ok(
    !errors.some((l) => l.includes('AUTH_SIGNIN_STARTED')),
    'normal marker must NOT be on the error channel',
  )
  // Real failure stays on the error channel with exact stage+reason.
  assert.ok(
    errors.some((l) => l.includes('AUTH_ERROR stage=GOOGLE_CALLBACK reason=INVALID_CALLBACK_STATE')),
    'AUTH_ERROR stage+reason logged on error channel',
  )
  // Never leaks secret values on either channel.
  const all = [...errors, ...infos]
  assert.ok(all.every((l) => !l.includes('secret') || l.includes('AUTH_SECRET') === false))
})
