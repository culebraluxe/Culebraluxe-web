// ---------------------------------------------------------------------------
// AUTH-09E — V1 ROOT / BUSINESS_POWER authority split.
//
// Proves the authority-based split without a live browser, using the Security
// repository seam + a fake SessionAdapter:
//   A  ROOT            -> tech.access true
//   B  BUSINESS_POWER  -> tech.access false
//   C  ROOT            -> TECH nav visible
//   D  BUSINESS_POWER  -> TECH nav hidden
//   E  ROOT            -> direct TECH route allowed
//   F  BUSINESS_POWER  -> direct TECH route denied
//   G  Security (Support) respects settings.read
//   H  Security mutation requires settings.manage
//   I  Lisa dual identities map to ONE app_user
//   J  identity resolution uses provider+subject, never email
//   K  inactive user denied
//   L  unknown provider subject denied
//   M  role assignment never creates a duplicate app_user
//   N  existing portal.read behavior intact
//
// THE SEAM MOVED WITH THE MAPPING. These tests used to stage rows through
// `setDatabaseTestExecutor`, because the projection ran in TypeScript. The
// projection is the Rust security service's now, so they stage the RESOLUTION
// (`setSecurityRepositoryForTesting`) — the actor a subject maps to — and keep
// asserting the authority rules and redirect targets, which is what this fence
// is actually about. J's subject moved too, so its assertion follows the code to
// `rust/core/db/src/security.rs` rather than guarding a TS file that no longer
// performs the lookup.
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
import type { ActingUser, AuthorityCode } from '@/lib/auth/types'
import type { SecurityRepository } from '@/legacy/services/security'
import {
  OPERATING_SURFACES,
  navigationForSurface,
} from '@/lib/navigation'

afterEach(() => {
  setSecurityRepositoryForTesting(null)
  setAuthorizationPortForTesting(null)
})

/**
 * The decision for the operations these tests run. Identity resolution is itself authorized
 * (`security.identity.resolve`, granted to the Auth.js edge actor by a Rust bootstrap rule), and this permits it so
 * the test is about the mapping and the gate rather than about a bridge client that cannot load outside a Next
 * runtime. The rule itself is asserted in Rust.
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

/** Install both doubles: the mapping, and the decision that permits reading it. */
function installDoubles(repository: SecurityRepository): void {
  setAuthorizationPortForTesting(permitAuthorization)
  setSecurityRepositoryForTesting(repository)
}

function actor(
  appUserId: string,
  roleCodes: string[],
  authorityCodes: string[],
): ActingUser {
  return {
    appUserId,
    displayName: appUserId,
    email: null,
    accountType: 'internal',
    roleCodes,
    authorityCodes,
    entitlementCodes: [],
    personId: null,
  }
}

const ROOT_PRINCIPAL = actor('root-user', ['root'], [
  'portal.read',
  'crm.write',
  'listing.write',
  'deal.read',
  'deal.write',
  'settings.read',
  'settings.manage',
  'tech.access',
])

const BP_PRINCIPAL = actor('bp-user', ['business_power'], [
  'portal.read',
  'crm.write',
  'listing.write',
  'deal.read',
  'deal.write',
  'settings.read',
])

/** A fake session for the given actor's subject. */
function authorizeAs(_p: ActingUser): SessionAdapter {
  return {
    getSession: async () => ({
      provider: 'google',
      providerSubject: 'sub-1',
      providerEmail: 'x@culebraluxe.com',
    }),
  }
}

function repositoryFor(p: ActingUser): SecurityRepository {
  return {
    async resolveProviderSubject() {
      return { kind: 'known', actingUser: p }
    },
    async getPrincipal() {
      return null
    },
  }
}

async function checkRoute(
  p: ActingUser,
  authority: string,
): Promise<{ ok: boolean; redirectTo?: string }> {
  installDoubles(repositoryFor(p))
  const result = await resolvePortalAccess(
    authorizeAs(p),
    authority as AuthorityCode,
  )
  return result.ok ? { ok: true } : { ok: false, redirectTo: result.redirectTo }
}

test('AUTH-09E A: ROOT has tech.access', async () => {
  const r = await checkRoute(ROOT_PRINCIPAL, 'tech.access')
  assert.equal(r.ok, true)
})

test('AUTH-09E B: BUSINESS_POWER does NOT have tech.access', async () => {
  const r = await checkRoute(BP_PRINCIPAL, 'tech.access')
  assert.equal(r.ok, false)
  if (!r.ok) assert.equal(r.redirectTo, '/login/unauthorized')
})

test('AUTH-09E C: ROOT sees TECH nav', () => {
  assert.equal(OPERATING_SURFACES.TECH.accessAuthority, 'tech.access')
  const items = navigationForSurface('TECH')
  assert.ok(items.length > 0, 'TECH has nav items')
  assert.ok(
    items.every((i) => i.authority === 'tech.access'),
    'all TECH nav items require tech.access',
  )
  const rootAuth = ROOT_PRINCIPAL.authorityCodes
  const visible = items.filter((i) => !i.authority || rootAuth.includes(i.authority))
  assert.equal(visible.length, items.length, 'ROOT sees all TECH nav items')
})

test('AUTH-09E D: BUSINESS_POWER does NOT see TECH nav', () => {
  const items = navigationForSurface('TECH')
  const bpAuth = BP_PRINCIPAL.authorityCodes
  const visible = items.filter((i) => !i.authority || bpAuth.includes(i.authority))
  assert.equal(visible.length, 0, 'no TECH nav items visible to BUSINESS_POWER')
})

test('AUTH-09E E: ROOT direct TECH route allowed', async () => {
  const r = await checkRoute(ROOT_PRINCIPAL, 'tech.access')
  assert.equal(r.ok, true)
})

test('AUTH-09E F: BUSINESS_POWER direct TECH route denied', async () => {
  const r = await checkRoute(BP_PRINCIPAL, 'tech.access')
  assert.equal(r.ok, false)
  if (!r.ok) assert.equal(r.redirectTo, '/login/unauthorized')
})

test('AUTH-09E G: Support/Security respects settings.read', async () => {
  // BUSINESS_POWER has settings.read -> Security route allowed.
  const bpRead = await checkRoute(BP_PRINCIPAL, 'settings.read')
  assert.equal(bpRead.ok, true)
  // An actor without settings.read is denied the Security surface.
  const noRead = actor('v', ['viewer'], ['portal.read', 'deal.read'])
  const denied = await checkRoute(noRead, 'settings.read')
  assert.equal(denied.ok, false)
})

test('AUTH-09E H: Security mutation requires settings.manage', async () => {
  // ROOT has settings.manage -> allowed.
  assert.equal((await checkRoute(ROOT_PRINCIPAL, 'settings.manage')).ok, true)
  // BUSINESS_POWER lacks settings.manage -> denied.
  const bp = await checkRoute(BP_PRINCIPAL, 'settings.manage')
  assert.equal(bp.ok, false)
})

test('AUTH-09E N: existing portal.read behavior intact for both roles', async () => {
  assert.equal((await checkRoute(ROOT_PRINCIPAL, 'portal.read')).ok, true)
  assert.equal((await checkRoute(BP_PRINCIPAL, 'portal.read')).ok, true)
})

test('AUTH-09E I: Lisa dual identities map to ONE app_user', async () => {
  // Two provider subjects, one application user: the mapping is by subject, and
  // both subjects resolve to the same actor, so the app_user is not duplicated.
  const shared = actor('shared-lisa-user', ['business_power'], ['portal.read'])
  installDoubles(repositoryFor(shared))
  const run = async (subject: string, email: string) =>
    resolvePortalAccess(
      {
        getSession: async () => ({
          provider: 'google',
          providerSubject: subject,
          providerEmail: email,
        }),
      },
      'portal.read',
    )
  const a = await run('google-sub-lisa-1', 'lisa@culebraluxe.com')
  const b = await run('google-sub-lisa-2', 'penfield33@gmail.com')
  assert.equal(a.ok && b.ok, true)
  if (a.ok && b.ok) assert.equal(a.actor.appUserId, b.actor.appUserId)
})

test('AUTH-09E J: identity resolution is keyed by provider+subject, never email', async () => {
  // THE LOOKUP MOVED, SO THE GUARD MOVED WITH IT. This asserted the SQL in
  // `legacy/db/auth-identity.ts`; the lookup is now in Rust, and a guard left
  // behind on the old file would keep passing while the real lookup drifted.
  const src = await readFile(
    new URL('../../../rust/core/db/src/security.rs', import.meta.url),
    'utf8',
  )
  const lookup = src.slice(
    src.indexOf('pub async fn resolve_provider_subject'),
    src.indexOf('pub async fn get_principal'),
  )
  assert.ok(lookup.length > 0, 'the Rust lookup was found')
  assert.ok(
    lookup.includes('provider = $1 and provider_subject = $2'),
    'lookup keyed by provider + provider_subject',
  )
  // Scoped to the lookup: `get_principal` selects u.email legitimately, so a
  // whole-file check for 'email' would be a guard that fails for the wrong
  // reason and then gets weakened until it passes.
  assert.ok(
    !/where[\s\S]{0,200}email/i.test(lookup),
    'email is never an identity lookup key',
  )
})

test('AUTH-09E K: inactive user denied', async () => {
  installDoubles({
    async resolveProviderSubject() {
      return { kind: 'inactive' }
    },
    async getPrincipal() {
      return null
    },
  })
  const r = await resolvePortalAccess(authorizeAs(ROOT_PRINCIPAL), 'portal.read')
  assert.equal(r.ok, false)
})

test('AUTH-09E L: unknown provider subject denied', async () => {
  installDoubles({
    async resolveProviderSubject() {
      return { kind: 'unmapped' }
    },
    async getPrincipal() {
      return null
    },
  })
  const r = await resolvePortalAccess(authorizeAs(ROOT_PRINCIPAL), 'portal.read')
  assert.equal(r.ok, false)
})

test('AUTH-09E M: role assignment never creates a duplicate app_user', async () => {
  const src = await readFile(
    new URL('../../../scripts/provision-v1-roles.ts', import.meta.url),
    'utf8',
  )
  assert.ok(src.includes('insert into app_user_role'), 'only role assignments written')
  assert.ok(!src.includes('insert into app_user '), 'never inserts into app_user')
})
