// ---------------------------------------------------------------------------
// AUTH-08G — Google subject -> auth_identity -> app_user -> authority mapping.
//
// Proves the application-authorization behavior for a GOOGLE-authenticated
// subject WITHOUT a live consent round-trip, using the DB gateway's test
// executor seam + a fake SessionAdapter (the provider side is already proven by
// the live /login -> /portal-auth-proof round-trip):
//
//   A  mapped google subject with portal.read  -> authorized /portal (ok:true)
//   B  unmapped google subject                -> /login/unauthorized
//   C  mapped but inactive app_user           -> denied (/login/unauthorized)
//   D  mapped but missing portal.read         -> denied (/login/unauthorized)
//   E  DB failure during authorization        -> denied cleanly; Auth.js
//                                                session stays valid; NO
//                                                Configuration error
//
// Authorization is deliberately OUTSIDE Auth.js: the SessionAdapter just hands
// the verified subject to getActingUser; auth.ts callbacks never touch the DB.
// ---------------------------------------------------------------------------

import { test, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import { setDatabaseTestExecutor } from '../../db/client'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import { resolvePortalAccess } from '../../lib/auth/require-portal-access'
import type { SessionAdapter } from '../../lib/auth/session-adapter'

afterEach(() => setDatabaseTestExecutor(null))

/** A Google-authenticated session (the stable provider `sub` is the key). */
function googleAdapter(subject = 'google:STABLE_SUBJECT'): SessionAdapter {
  return {
    getSession: async () => ({
      provider: 'google',
      providerSubject: subject,
      providerEmail: 'admin@culebraluxe.com',
    }),
  }
}

/** Canned row-sets replayed in order (last set repeats). */
function makeExecutor(sequences: QueryRow[][]): QueryExecutor {
  let i = 0
  return async (strings) => {
    const set = sequences[Math.min(i, sequences.length - 1)] ?? []
    i++
    return set
  }
}

/** A valid owner principal projection (getSecurityPrincipal row shape). */
function ownerPrincipal(overrides: Partial<QueryRow> = {}): QueryRow {
  return {
    app_user_id: 'user-1',
    display_name: 'Lisa Penfield',
    email: 'lisa@culebraluxe.com',
    account_type: 'internal',
    person_id: null,
    role_codes: ['owner'],
    authority_codes: [
      'portal.read',
      'crm.write',
      'listing.write',
      'deal.read',
      'deal.write',
      'settings.read',
      'settings.manage',
    ],
    ...overrides,
  }
}

test('AUTH-08G A: mapped google subject with portal.read is authorized (/portal)', async () => {
  // identity lookup -> [app_user_id], principal projection -> owner row.
  setDatabaseTestExecutor(
    makeExecutor([[{ app_user_id: 'user-1' }], [ownerPrincipal()]]),
  )
  const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
  assert.equal(result.ok, true)
  if (result.ok) {
    assert.equal(result.actor.appUserId, 'user-1')
    assert.ok(result.actor.authorityCodes.includes('portal.read'))
  }
})

test('AUTH-08G B: unmapped google subject is denied (/login/unauthorized)', async () => {
  setDatabaseTestExecutor(makeExecutor([[]]))
  const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
  assert.equal(result.ok, false)
  if (!result.ok) assert.equal(result.redirectTo, '/login/unauthorized')
})

test('AUTH-08G C: mapped but INACTIVE app_user is denied', async () => {
  // identity maps to a user, but the principal projection returns no row
test('AUTH-08G E: DB failure during authorization denies cleanly (session valid, no Configuration)', async () => {
  // Gateway logs a [db:gateway] failure but the mapping FAILS CLOSED to unmapped.
  const failingExecutor: QueryExecutor = async () => {
    const e = new Error('connection failure (test)')
    ;(e as { code?: string }).code = '08006' // connection_failure
    throw e
  }
  setDatabaseTestExecutor(failingExecutor)

  const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
  // Denied cleanly — a typed result, never a Configuration throw.
  assert.equal(result.ok, false)
  if (!result.ok) assert.equal(result.redirectTo, '/login/unauthorized')

  // Auth.js session independence: the Google baseline never touches the DB, so
  // an authorization DB failure cannot invalidate authentication or surface as
  // an Auth.js Configuration error. Assert the source guarantee.
  const authSrc = await readFile(new URL('../../auth.ts', import.meta.url), 'utf8')
  // Code only (comments stripped) so the guard targets real statements.
  const authCode = authSrc
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/^\s*\/\/.*$/gm, '')
  assert.ok(!authCode.includes('@/db/'), 'auth.ts never imports the DB')
  assert.ok(!authCode.includes('authorization'), 'no authorization inside Auth.js')
  assert.ok(!authCode.includes('app_user'), 'no app_user lookup inside Auth.js')
})

test('AUTH-08G: DEV flight markers emitted for the identity mapping (lookup + mapped)', async () => {
  setDatabaseTestExecutor(
    makeExecutor([[{ app_user_id: 'user-1' }], [ownerPrincipal()]]),
  )
  const captured: string[] = []
  const origErr = console.error
  const origInfo = console.info
  console.error = (...args: unknown[]) => captured.push(String(args[0]))
  console.info = (...args: unknown[]) => captured.push(String(args[0]))
  try {
    const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
    assert.equal(result.ok, true)
  } finally {
    console.error = origErr
    console.info = origInfo
  }
  assert.ok(
    captured.some((l) => l.includes('AUTH_APP_IDENTITY_LOOKUP_STARTED')),
    'lookup-started marker emitted',
  )
  assert.ok(
    captured.some((l) => l.includes('AUTH_APP_IDENTITY_MAPPED')),
    'mapped marker emitted',
  )
  assert.ok(
    captured.some((l) => l.includes('AUTH_PORTAL_AUTHORIZED')),
    'portal-authorized marker emitted',
  )
})

  // (getSecurityPrincipal returns null for inactive users -> denied).
  setDatabaseTestExecutor(
    makeExecutor([[{ app_user_id: 'user-1' }], []]),
  )
  const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
  assert.equal(result.ok, false)
  if (!result.ok) assert.equal(result.redirectTo, '/login/unauthorized')
})

test('AUTH-08G D: mapped but MISSING portal.read is denied', async () => {
  setDatabaseTestExecutor(
    makeExecutor([
      [{ app_user_id: 'user-1' }],
      [ownerPrincipal({ role_codes: ['viewer'], authority_codes: ['deal.read'] })],
    ]),
  )
  const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
  assert.equal(result.ok, false)
  if (!result.ok) assert.equal(result.redirectTo, '/login/unauthorized')
})
