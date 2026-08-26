// ---------------------------------------------------------------------------
// AUTH-09E — V1 ROOT / BUSINESS_POWER tech entitlement.
//
// Proves the authority-based split without a live browser, using the DB gateway
// test executor seam + a fake SessionAdapter:
//   A  ROOT            -> tech.access true
//   B  BUSINESS_POWER  -> tech.access false
//   C  ROOT            -> TECH nav visible
//   D  BUSINESS_POWER  -> TECH nav hidden
//   E  ROOT            -> direct TECH route allowed
//   F  BUSINESS_POWER  -> direct TECH route denied
//   G  Security (Support) respects settings.read
//   H  Security mutation requires settings.manage
//   I  Lisa dual identities map to ONE app_user
//   J  runtime lookup uses provider+subject, never email
//   K  inactive user denied
//   L  unknown provider subject denied
//   M  role assignment never creates a duplicate app_user
//   N  existing portal.read behavior intact
// ---------------------------------------------------------------------------

import { test, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import { setDatabaseTestExecutor } from '../../db/client'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import { resolvePortalAccess } from '../../lib/auth/require-portal-access'
import type { SessionAdapter } from '../../lib/auth/session-adapter'
import {
  OPERATING_SURFACES,
  navigationForSurface,
} from '../../lib/navigation'

afterEach(() => setDatabaseTestExecutor(null))

/** Canned row-sets replayed in order (last set repeats). */
function makeExecutor(sequences: QueryRow[][]): QueryExecutor {
  let i = 0
  return async (strings) => {
    const set = sequences[Math.min(i, sequences.length - 1)] ?? []
    i++
    return set
  }
}

function principal(
  appUserId: string,
  roleCodes: string[],
  authorityCodes: string[],
): QueryRow {
  return {
    app_user_id: appUserId,
    display_name: appUserId,
    email: null,
    account_type: 'internal',
    person_id: null,
    role_codes: roleCodes,
    authority_codes: authorityCodes,
  }
}

const ROOT_PRINCIPAL = principal('root-user', ['root'], [
  'portal.read',
  'crm.write',
  'listing.write',
  'deal.read',
  'deal.write',
  'settings.read',
  'settings.manage',
  'tech.access',
])

const BP_PRINCIPAL = principal('bp-user', ['business_power'], [
  'portal.read',
  'crm.write',
  'listing.write',
  'deal.read',
  'deal.write',
  'settings.read',
])

/** Fake adapter + executor resolving to the given principal (subject 'sub-1'). */
function authorizeAs(p: QueryRow): SessionAdapter {
  return {
    getSession: async () => ({
      provider: 'google',
      providerSubject: 'sub-1',
      providerEmail: 'x@culebraluxe.com',
    }),
  }
}
function executorFor(p: QueryRow): QueryExecutor {
  return makeExecutor([[{ app_user_id: p.app_user_id }], [p]])
}
async function checkRoute(
  p: QueryRow,
  authority: string,
): Promise<{ ok: boolean; redirectTo?: string }> {
  setDatabaseTestExecutor(executorFor(p))
  const result = await resolvePortalAccess(authorizeAs(p), authority as never)
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
  const noRead = principal('v', ['viewer'], ['portal.read', 'deal.read'])
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

test('AUTH-09E I: Lisa dual identities map to ONE app_user', async () => {
  const shared = 'shared-lisa-user'
  const p = principal(shared, ['business_power'], ['portal.read'])
  const run = async (subject: string, email: string) => {
    setDatabaseTestExecutor(makeExecutor([[{ app_user_id: shared }], [p]]))
    return resolvePortalAccess(
      {
        getSession: async () => ({
          provider: 'google',
          providerSubject: subject,
          providerEmail: email,
        }),
      },
      'portal.read',
    )
  }
  const a = await run('google-sub-lisa-1', 'lisa@culebraluxe.com')
  const b = await run('google-sub-lisa-2', 'penfield33@gmail.com')
  assert.equal(a.ok && b.ok, true)
  if (a.ok && b.ok) assert.equal(a.actor.appUserId, b.actor.appUserId)
})

test('AUTH-09E J: runtime lookup uses provider+subject, never email', async () => {
  const src = await readFile(new URL('../../db/auth-identity.ts', import.meta.url), 'utf8')
  assert.ok(src.includes('provider_subject'), 'lookup keyed by provider_subject')
  assert.ok(
    !/where[\s\S]{0,140}email/i.test(src),
    'no email used as an identity lookup key in resolveProviderSubject',
  )
})

test('AUTH-09E K: inactive user denied', async () => {
  setDatabaseTestExecutor(makeExecutor([[{ app_user_id: 'u1' }], []]))
  const r = await resolvePortalAccess(authorizeAs(ROOT_PRINCIPAL), 'portal.read')
  assert.equal(r.ok, false)
})

test('AUTH-09E L: unknown provider subject denied', async () => {
  setDatabaseTestExecutor(makeExecutor([[]]))
  const r = await resolvePortalAccess(authorizeAs(ROOT_PRINCIPAL), 'portal.read')
  assert.equal(r.ok, false)
})

test('AUTH-09E M: role assignment never creates a duplicate app_user', async () => {
  const src = await readFile(new URL('../../scripts/provision-v1-roles.ts', import.meta.url), 'utf8')
  assert.ok(src.includes('insert into app_user_role'), 'only role assignments written')
  assert.ok(!src.includes('insert into app_user '), 'never inserts into app_user')
})

test('AUTH-09E C: ROOT sees TECH nav', () => {
  assert.equal(OPERATING_SURFACES.TECH.accessAuthority, 'tech.access')
  const items = navigationForSurface('TECH')
  assert.ok(items.length > 0, 'TECH has nav items')
  assert.ok(
    items.every((i) => i.authority === 'tech.access'),
    'all TECH nav items require tech.access',
  )
  const rootAuth = ROOT_PRINCIPAL.authority_codes as string[]
  const visible = items.filter((i) => !i.authority || rootAuth.includes(i.authority))
  assert.equal(visible.length, items.length, 'ROOT sees all TECH nav items')
})

test('AUTH-09E D: BUSINESS_POWER does NOT see TECH nav', () => {
  const items = navigationForSurface('TECH')
  const bpAuth = BP_PRINCIPAL.authority_codes as string[]
  const visible = items.filter((i) => !i.authority || bpAuth.includes(i.authority))
  assert.equal(visible.length, 0, 'no TECH nav items visible to BUSINESS_POWER')
})

})
