// ---------------------------------------------------------------------------
// AUTH-08G — Google subject -> application actor, and the Portal gate that uses it.
//
// Proves the application-authorization behavior for a GOOGLE-authenticated
// subject WITHOUT a live consent round-trip, using the Security repository seam
// + a fake SessionAdapter (the provider side is already proven by the live
// /login -> /portal-auth-proof round-trip):
//
//   A  mapped google subject with portal.read  -> authorized /portal (ok:true)
//   B  unmapped google subject                 -> /login/unauthorized
//   C  mapped but inactive app_user            -> denied (/login/unauthorized)
//   D  mapped but missing portal.read          -> denied (/login/unauthorized)
//   E  mapping failure during authorization    -> denied cleanly; Auth.js
//                                                session stays valid; NO
//                                                Configuration error
//
// WHERE THE SEAM IS, AND WHY IT MOVED. These tests used to stage auth_identity /
// app_user rows through `setDatabaseTestExecutor`, because the mapping WAS those
// queries in TypeScript. The mapping is the Rust security service's now (one
// identity engine), so the same test stages the RESOLUTION instead
// (`setSecurityRepositoryForTesting`) and keeps asserting what TypeScript still
// owns: getActingUser's error mapping, the authority requirement, and the
// redirect targets. A test pinned to SQL spends its assertions on how a boundary
// is implemented rather than on the boundary's contract — which is exactly why
// this one broke when the implementation moved, having never tested the contract.
//
// Authorization is deliberately OUTSIDE Auth.js: the SessionAdapter just hands
// the verified subject to getActingUser; auth.ts callbacks never touch the DB.
// ---------------------------------------------------------------------------

import { test, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import {
  setAuthorizationPortForTesting,
  setSecurityRepositoryForTesting,
} from '@/lib/auth/security-runtime'
import type { SessionAdapter } from '@/lib/auth/session-adapter'
import type { ActingUser } from '@/lib/auth/types'
import type {
  SecurityRepository,
  SecurityRepositoryIdentityResolution,
} from '@/legacy/services/security'

afterEach(() => {
  setSecurityRepositoryForTesting(null)
  setAuthorizationPortForTesting(null)
})

/**
 * The decision for the operations these tests run. The login seam's resolution is
 * itself authorized (`security.identity.resolve`, granted to the Auth.js edge
 * actor by a Rust bootstrap rule), and this permits it so the test is about the
 * MAPPING and the gate rather than about a bridge client that cannot load outside
 * a Next runtime. The rule itself is asserted in Rust.
 */
const permitAuthorization = {
  mode: 'enforced' as const,
  authorize: async () => ({
    allowed: true,
    reason: 'test: identity resolution is permitted',
    policyId: 'test:permit',
    mode: 'enforced' as const,
  }),
}

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

/** A valid owner actor (the application projection of a mapped subject). */
function ownerActor(overrides: Partial<ActingUser> = {}): ActingUser {
  return {
    appUserId: 'user-1',
    displayName: 'Lisa Penfield',
    email: 'lisa@culebraluxe.com',
    accountType: 'internal',
    roleCodes: ['owner'],
    authorityCodes: [
      'portal.read',
      'crm.write',
      'listing.write',
      'deal.read',
      'deal.write',
      'settings.read',
      'settings.manage',
    ],
    entitlementCodes: [],
    personId: null,
    ...overrides,
  }
}

/**
 * Stage the RESOLUTION: the one thing these tests need to control is "which actor
 * does this subject map to, if any". An Error stands for a repository that cannot
 * answer, which is the fail-closed case.
 */
function resolveAs(
  resolution: SecurityRepositoryIdentityResolution | Error,
): void {
  setAuthorizationPortForTesting(permitAuthorization)
  setSecurityRepositoryForTesting({
    async resolveProviderSubject() {
      if (resolution instanceof Error) throw resolution
      return resolution
    },
    async getPrincipal() {
      return null
    },
  })
}
test('AUTH-08G A: mapped google subject with portal.read is authorized (/portal)', async () => {
  resolveAs({ kind: 'known', actingUser: ownerActor() })
  const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
  assert.equal(result.ok, true)
  if (result.ok) {
    assert.equal(result.actor.appUserId, 'user-1')
    assert.ok(result.actor.authorityCodes.includes('portal.read'))
  }
})

test('AUTH-08G B: unmapped google subject is denied (/login/unauthorized)', async () => {
  resolveAs({ kind: 'unmapped' })
  const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
  assert.equal(result.ok, false)
  if (!result.ok) assert.equal(result.redirectTo, '/login/unauthorized')
})

test('AUTH-08G C: mapped but INACTIVE app_user is denied', async () => {
  // The subject maps, but the account is not active. The service says so as
  // data ('inactive') rather than as an error, because that is a different
  // thing to tell a person than 'unmapped' — and both are denied.
  resolveAs({ kind: 'inactive' })
  const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
  assert.equal(result.ok, false)
  if (!result.ok) assert.equal(result.redirectTo, '/login/unauthorized')
})

test('AUTH-08G D: mapped but MISSING portal.read is denied', async () => {
  resolveAs({
    kind: 'known',
    actingUser: ownerActor({
      roleCodes: ['viewer'],
      authorityCodes: ['deal.read'],
    }),
  })
  const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
  assert.equal(result.ok, false)
  if (!result.ok) assert.equal(result.redirectTo, '/login/unauthorized')
})

test('AUTH-08G E: a mapping failure denies cleanly (session valid, no Configuration)', async () => {
  // The repository cannot answer (connection failure): the mapping FAILS CLOSED.
  const failure = new Error('connection failure (test)')
  ;(failure as { code?: string }).code = '08006' // connection_failure
  resolveAs(failure)

  const result = await resolvePortalAccess(googleAdapter(), 'portal.read')
  // Denied cleanly — a typed result, never a Configuration throw.
  assert.equal(result.ok, false)
  if (!result.ok) assert.equal(result.redirectTo, '/login/unauthorized')

  // Auth.js session independence: the Auth.js path never touches the DB, so an
  // authorization failure cannot invalidate authentication or surface as an
  // Auth.js Configuration error. Assert the source guarantee.
  const authSrc = await readFile(new URL('../../../auth.ts', import.meta.url), 'utf8')
  // Code only (comments stripped) so the guard targets real statements.
  const authCode = authSrc
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/^\s*\/\/.*$/gm, '')
  assert.ok(!authCode.includes('@/db/'), 'auth.ts never imports the DB')
  assert.ok(!authCode.includes('authorization'), 'no authorization inside Auth.js')
  assert.ok(!authCode.includes('app_user'), 'no app_user lookup inside Auth.js')
})

test('AUTH-08G: DEV flight markers emitted for the identity mapping (lookup + mapped)', async () => {
  resolveAs({ kind: 'known', actingUser: ownerActor() })
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

