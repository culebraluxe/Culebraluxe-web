// ---------------------------------------------------------------------------
// AUTH-02 — Portal authorization (targeted).
//
// SCOPED tests only: the changed seams (route-policy + middleware decision
// logic, authoritative resolvePortalAccess guard, capability snapshot, external
// deal read scoping) and directly adjacent security layers (authority matrix,
// UI nav projection). No full harness, no unrelated regression.
//
// Pure tests (no DB): route-policy mapping, middleware decision matrix,
// authority helpers, portal-nav UI filtering, actor snapshot projection.
// Persistence tests (DEV Neon branch via db/client): the authoritative
// server-side guard (resolvePortalAccess) against the canonical projection,
// the sign-in capability snapshot, and external deal.read_own row scoping,
// using tunit- prefixed fixtures removed in after(). Requires the AUTH-01 DEV
// bootstrap (owner role + break-glass identity on lisa@culebraluxe.com) — see
// docs/auth-bootstrap-order.md.
// ---------------------------------------------------------------------------

import { test, before, after } from 'node:test'
import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'

import { sql } from '../../../db/client'
import { authoritiesForPath, isPublicAuthPath } from '../../../lib/auth/route-policy'
import { decidePortalMiddleware } from '../../../lib/auth/middleware-policy'
import { hasAuthority, requireAuthority } from '../../../lib/auth/authority'
import { MissingAuthorityError } from '../../../lib/auth/errors'
import { filterPortalNavigation, PORTAL_NAVIGATION } from '../../../lib/auth/portal-navigation'
import { toPortalActorSnapshot } from '../../../lib/auth/actor-snapshot'
import { resolvePortalAccess } from '../../../lib/auth/require-portal-access'
import { getSessionAuthoritySnapshot } from '../../../lib/auth/session-capability-snapshot'
import { getDeals } from '../../../db/deals'
import { getDealWorkspace } from '../../../db/deal-workspace'
import { breakGlassSubject } from '../../../auth'
import type { AuthenticatedIdentity } from '../../../lib/auth/types'
import type { SessionAdapter } from '../../../lib/auth/session-adapter'
import type { ActingUser } from '../../../lib/auth/types'

// ---------------------------------------------------------------------------
// DEV bootstrap constants (docs/auth-bootstrap-order.md)
// ---------------------------------------------------------------------------

const ROOT_USER_ID = 'aa06d089-162c-4bef-84ec-a76ee38cc8ad'
const ROOT_BREAK_GLASS_SUBJECT = breakGlassSubject(ROOT_USER_ID)

const OWNER_AUTHORITIES = [
  'crm.write',
  'deal.read',
  'deal.write',
  'listing.write',
  'portal.read',
  'settings.manage',
  'settings.read',
]
const VIEWER_AUTHORITIES = ['deal.read', 'portal.read']
const CLIENT_AUTHORITIES = ['external.deal.read_own', 'external.properties.save']

// ---------------------------------------------------------------------------
// Fixtures (tunit- prefixed; removed in after())
// ---------------------------------------------------------------------------

type FixtureUser = { id: string; subject: string }

const fixtures: FixtureUser[] = []
const fixtureDeals: { dealId: string; propertyId: string; personId: string }[] = []
const fixturePersonIds: string[] = []
const fixturePropertyIds: string[] = []

async function createFixtureUser(opts: {
  displayName: string
  accountType: 'internal' | 'external'
  active: boolean
  roleCode: string | null
}): Promise<FixtureUser> {
  const email = `tunit-auth02-${randomUUID()}@test.local`
  const subject = `tunit-google-${opts.displayName.toLowerCase().replace(/\s+/g, '-')}-${randomUUID()}`
  const rows = await sql`
    insert into app_user (display_name, email, account_type, active)
    values (${opts.displayName}, ${email}, ${opts.accountType}, ${opts.active})
    returning id
  `
  const id = (rows[0] as { id: string }).id
  if (opts.roleCode) {
    await sql`
      insert into app_user_role (app_user_id, role_id, assigned_by_user_id)
      select ${id}, r.id, null
      from role r
      where r.code = ${opts.roleCode}
    `
  }
  await sql`
    insert into auth_identity (app_user_id, provider, provider_subject, provider_email)
    values (${id}, 'google', ${subject}, null)
  `
  const fixture = { id, subject }
  fixtures.push(fixture)
  return fixture
}

async function createFixturePerson(displayName: string): Promise<string> {
  const rows = await sql`
    insert into person (display_name, role, status)
    values (${displayName}, 'buyer', 'active')
    returning id
  `
  const id = (rows[0] as { id: string }).id
  fixturePersonIds.push(id)
  return id
}

async function createFixtureDealForPerson(personId: string): Promise<string> {
  const propertyRows = await sql`
    insert into property (name, location)
    values ('tunit-property', 'Culebra')
    returning id
  `
  const propertyId = (propertyRows[0] as { id: string }).id
  fixturePropertyIds.push(propertyId)

  const dealRows = await sql`
    insert into deal (property_id, client_person_id, stage)
    values (${propertyId}, ${personId}, 'showing')
    returning id
  `
  const dealId = (dealRows[0] as { id: string }).id

  await sql`
    insert into deal_participant (deal_id, person_id, role, active)
    values (${dealId}, ${personId}, 'client', true)
  `

  fixtureDeals.push({ dealId, propertyId, personId })
  return dealId
}

function stubAdapter(session: AuthenticatedIdentity | null): SessionAdapter {
  return { getSession: async () => session }
}

let viewer: FixtureUser
let client: FixtureUser
let inactiveUser: FixtureUser

let personA: string
let personB: string
let dealA: string
let dealB: string

before(async () => {
  viewer = await createFixtureUser({ displayName: 'Viewer', accountType: 'internal', active: true, roleCode: 'viewer' })
  client = await createFixtureUser({ displayName: 'Client', accountType: 'external', active: true, roleCode: 'client' })
  inactiveUser = await createFixtureUser({ displayName: 'Inactive', accountType: 'internal', active: false, roleCode: 'viewer' })

  // External actor linked to person A; dealA belongs to person A, dealB to person B.
  personA = await createFixturePerson('tunit person A')
  personB = await createFixturePerson('tunit person B')
  await sql`update app_user set person_id = ${personA} where id = ${client.id}`
  dealA = await createFixtureDealForPerson(personA)
  dealB = await createFixtureDealForPerson(personB)
})

after(async () => {
  await sql`
    delete from deal_participant
    where deal_id in (select id from deal where id = any(${fixtureDeals.map((d) => d.dealId)}::uuid[]))
  `
  await sql`
    delete from deal where id = any(${fixtureDeals.map((d) => d.dealId)}::uuid[])
  `
  await sql`
    delete from property where id = any(${fixturePropertyIds}::uuid[])
  `
  await sql`
    delete from person where id = any(${fixturePersonIds}::uuid[])
  `
  await sql`delete from auth_identity where provider_subject like 'tunit-%'`
  await sql`
    delete from app_user_role
    where app_user_id in (select id from app_user where email like 'tunit-auth02-%@test.local')
  `
  await sql`delete from app_user where email like 'tunit-auth02-%@test.local'`
})

// ---------------------------------------------------------------------------
// Pure — route-policy mapping (coarse authorities, most-specific first)
// ---------------------------------------------------------------------------

test('AUTH-02: PORTAL_ROUTE_POLICY maps portal paths to coarse authorities (settings most-specific)', () => {
  assert.deepEqual(authoritiesForPath('/portal/dashboard'), ['portal.read'])
  assert.deepEqual(authoritiesForPath('/portal'), ['portal.read'])
  assert.deepEqual(authoritiesForPath('/portal/deals/123'), ['portal.read'])
  assert.deepEqual(authoritiesForPath('/portal/settings'), ['settings.read'])
  assert.deepEqual(authoritiesForPath('/portal/settings/users'), ['settings.read'])
  // Prefix does not bleed: /portal/settingsx is a portal page, not a settings page.
  assert.deepEqual(authoritiesForPath('/portal/settingsx'), ['portal.read'])
  // Auth/public paths are outside the policy.
  assert.equal(authoritiesForPath('/login'), null)
  assert.equal(authoritiesForPath('/api/auth/session'), null)
  assert.equal(authoritiesForPath('/properties'), null)
  assert.equal(isPublicAuthPath('/login'), true)
  assert.equal(isPublicAuthPath('/login/unauthorized'), true)
  assert.equal(isPublicAuthPath('/api/auth/callback/google'), true)
  assert.equal(isPublicAuthPath('/portal/dashboard'), false)
})

// ---------------------------------------------------------------------------
// Pure — middleware decision matrix (cheap Edge gate; never authoritative)
// ---------------------------------------------------------------------------

test('AUTH-02: middleware redirects unauthenticated portal requests to /login', () => {
  const decision = decidePortalMiddleware('/portal/dashboard', { authenticated: false })
  assert.deepEqual(decision, { kind: 'redirect', to: '/login' })
})

test('AUTH-02: middleware redirects authenticated-but-missing portal.read to /login/unauthorized', () => {
  const decision = decidePortalMiddleware('/portal/dashboard', {
    authenticated: true,
    capabilities: ['deal.read'],
  })
  assert.deepEqual(decision, { kind: 'redirect', to: '/login/unauthorized' })
})

test('AUTH-02: middleware passes authenticated portal requests with the required authority', () => {
  const decision = decidePortalMiddleware('/portal/dashboard', {
    authenticated: true,
    capabilities: ['portal.read'],
  })
  assert.deepEqual(decision, { kind: 'next' })
})

test('AUTH-02: /portal/settings additionally requires settings.read at the middleware gate', () => {
  const viewerCaps = ['portal.read', 'deal.read']
  assert.deepEqual(decidePortalMiddleware('/portal/settings', { authenticated: true, capabilities: viewerCaps }), {
    kind: 'redirect',
    to: '/login/unauthorized',
  })
  const ownerCaps = [...OWNER_AUTHORITIES]
  assert.deepEqual(decidePortalMiddleware('/portal/settings', { authenticated: true, capabilities: ownerCaps }), {
    kind: 'next',
  })
})

test('AUTH-02: middleware never denies on a missing capability snapshot (authoritative check is server-side)', () => {
  // Sessions minted before the snapshot claim, or with an unmapped identity,
  // carry no capabilities. The cheap gate must pass through — the server-side
  // resolvePortalAccess/getActingUser guard is the authoritative decider.
  assert.deepEqual(
    decidePortalMiddleware('/portal/dashboard', { authenticated: true, capabilities: null }),
    { kind: 'next' },
  )
  assert.deepEqual(
    decidePortalMiddleware('/portal/dashboard', { authenticated: true }),
    { kind: 'next' },
  )
})

test('AUTH-02: middleware ignores public auth paths even when unauthenticated', () => {
  assert.deepEqual(decidePortalMiddleware('/login', { authenticated: false }), { kind: 'next' })
  assert.deepEqual(decidePortalMiddleware('/login/unauthorized', { authenticated: false }), { kind: 'next' })
  assert.deepEqual(decidePortalMiddleware('/api/auth/callback/google', { authenticated: false }), { kind: 'next' })
})

// ---------------------------------------------------------------------------
// Pure — authority helpers (exact match, no wildcard)
// ---------------------------------------------------------------------------

test('AUTH-02: hasAuthority/requireAuthority are exact-code checks', () => {
  const actor = {
    appUserId: 'u1',
    displayName: 'T',
    email: null,
    accountType: 'internal',
    roleCodes: ['viewer'],
    authorityCodes: [...VIEWER_AUTHORITIES],
    personId: null,
  } as ActingUser
  assert.equal(hasAuthority(actor, 'portal.read'), true)
  assert.equal(hasAuthority(actor, 'settings.read'), false)
  assert.throws(() => requireAuthority(actor, 'settings.read'), MissingAuthorityError)
  assert.equal(hasAuthority(actor, '*'), false, 'no wildcard authority may ever match')
})

// ---------------------------------------------------------------------------
// Pure — UI nav projection (cosmetic; never the security boundary)
// ---------------------------------------------------------------------------

test('AUTH-02: sidebar nav hides Settings from viewers and Deals stays visible (deal.read)', () => {
  const viewerNav = filterPortalNavigation(VIEWER_AUTHORITIES)
  const labels = viewerNav.flatMap((g) => g.items.map((i) => i.label))
  assert.ok(labels.includes('Deals'), 'viewer holds deal.read and sees Deals')
  assert.ok(!labels.includes('Settings'), 'viewer lacks settings.read and Settings is hidden')
  assert.ok(labels.includes('Dashboard'), 'portal.read items stay visible')
})

test('AUTH-02: sidebar nav shows Settings only to settings.read holders and nothing to external clients', () => {
  const ownerNav = filterPortalNavigation(OWNER_AUTHORITIES)
  assert.ok(ownerNav.flatMap((g) => g.items.map((i) => i.label)).includes('Settings'))

  const clientNav = filterPortalNavigation(CLIENT_AUTHORITIES)
  assert.deepEqual(clientNav, [], 'external client authorities match no portal nav item')
})

test('AUTH-02: actor snapshot is the narrow serializable projection', () => {
  const actor = {
    appUserId: 'u1',
    displayName: 'Lisa',
    email: 'lisa@example.com',
    accountType: 'internal',
    roleCodes: ['owner'],
    authorityCodes: [...OWNER_AUTHORITIES],
    personId: null,
  } as ActingUser
  const snapshot = toPortalActorSnapshot(actor)
  assert.deepEqual(snapshot, {
    displayName: 'Lisa',
    accountType: 'internal',
    authorityCodes: [...OWNER_AUTHORITIES],
  })
  assert.ok(!('appUserId' in snapshot) && !('email' in snapshot), 'no session/account internals leak to the client')
})

// ---------------------------------------------------------------------------
// Persistence — sign-in capability snapshot (what the jwt callback stamps)
// ---------------------------------------------------------------------------

test('AUTH-02: the capability snapshot matches the canonical projection (viewer)', async () => {
  const caps = await getSessionAuthoritySnapshot('google', viewer.subject)
  assert.deepEqual(caps, VIEWER_AUTHORITIES)
})

test('AUTH-02: an external client snapshot contains no portal.read', async () => {
  const caps = await getSessionAuthoritySnapshot('google', client.subject)
  assert.deepEqual(caps, CLIENT_AUTHORITIES)
  assert.ok(!caps!.includes('portal.read'), 'external client must not gain portal.read')
})

test('AUTH-02: unmapped and inactive identities produce no snapshot (fail closed)', async () => {
  assert.equal(await getSessionAuthoritySnapshot('google', `tunit-google-unknown-${randomUUID()}`), null)
  assert.equal(await getSessionAuthoritySnapshot('google', inactiveUser.subject), null)
  assert.equal(await getSessionAuthoritySnapshot('', ''), null)
})

// ---------------------------------------------------------------------------
// Persistence — authoritative server-side guard (resolvePortalAccess)
// ---------------------------------------------------------------------------

test('AUTH-02: unauthenticated portal access resolves to /login', async () => {
  const result = await resolvePortalAccess(stubAdapter(null), 'portal.read')
  assert.deepEqual(result, { ok: false, redirectTo: '/login' })
})

test('AUTH-02: viewer passes portal.read but is denied settings.read (redirect /login/unauthorized)', async () => {
  const viewerAdapter = stubAdapter({ provider: 'google', providerSubject: viewer.subject, providerEmail: null })
  const portal = await resolvePortalAccess(viewerAdapter, 'portal.read')
  assert.ok(portal.ok)
  if (!portal.ok) return
  assert.equal(portal.actor.accountType, 'internal')

  const settings = await resolvePortalAccess(viewerAdapter, 'settings.read')
  assert.deepEqual(settings, { ok: false, redirectTo: '/login/unauthorized' })
})

test('AUTH-02: an authenticated external client is denied portal.read server-side', async () => {
  const clientAdapter = stubAdapter({ provider: 'google', providerSubject: client.subject, providerEmail: null })
  const result = await resolvePortalAccess(clientAdapter, 'portal.read')
  assert.deepEqual(result, { ok: false, redirectTo: '/login/unauthorized' })
})

test('AUTH-02: owner resolves settings.read through the canonical projection, no wildcard', async () => {
  const ownerAdapter = stubAdapter({
    provider: 'break-glass',
    providerSubject: ROOT_BREAK_GLASS_SUBJECT,
    providerEmail: null,
  })
  const settings = await resolvePortalAccess(ownerAdapter, 'settings.read')
  assert.ok(settings.ok)
  if (!settings.ok) return
  assert.deepEqual(settings.actor.authorityCodes, OWNER_AUTHORITIES)
  assert.ok(!settings.actor.authorityCodes.includes('*'), 'no wildcard authority may exist')
  // The owner is denied any authority NOT in the explicit seeded set (matrix O).
  assert.throws(
    () => requireAuthority(settings.actor, 'external.properties.save'),
    MissingAuthorityError,
    'owner must not bypass authorities it lacks',
  )
})

// ---------------------------------------------------------------------------
// Persistence — external deal.read_own row scoping
// ---------------------------------------------------------------------------

test('AUTH-02: an external actor reads only deals linked to their own person', async () => {
  const ownDeals = await getDeals({ accountType: 'external', personId: personA })
  const ownIds = ownDeals.map((d) => d.id)
  assert.ok(ownIds.includes(dealA), 'deal linked to the actor person is visible')
  assert.ok(!ownIds.includes(dealB), 'deal linked to another person is NOT visible')
})

test('AUTH-02: an external actor with no linked person reads no deals (fail closed)', async () => {
  const noPersonDeals = await getDeals({ accountType: 'external', personId: null })
  assert.deepEqual(noPersonDeals, [])
})

test('AUTH-02: an internal coarse read is not scoped (sees both fixture deals)', async () => {
  const allDeals = await getDeals()
  const ids = allDeals.map((d) => d.id)
  assert.ok(ids.includes(dealA))
  assert.ok(ids.includes(dealB))
})

test('AUTH-02: external deal workspace is scoped and foreign deals return the empty workspace', async () => {
  const externalA = { accountType: 'external' as const, personId: personA }

  const ownWorkspace = await getDealWorkspace(dealA, externalA)
  assert.equal(ownWorkspace.deal?.id, dealA)

  const foreignWorkspace = await getDealWorkspace(dealB, externalA)
  assert.equal(foreignWorkspace.deal, null)
  assert.deepEqual(foreignWorkspace.participants, [])
  assert.deepEqual(foreignWorkspace.offers, [])

  // Internal (no actor) coarse read still sees the deal.
  const internalWorkspace = await getDealWorkspace(dealB)
  assert.equal(internalWorkspace.deal?.id, dealB)
})

// ---------------------------------------------------------------------------
// Persistence — portal nav availability of every link target
// ---------------------------------------------------------------------------

test('AUTH-02: every portal nav item points at a route covered by PORTAL_ROUTE_POLICY', () => {
  for (const group of PORTAL_NAVIGATION) {
    for (const item of group.items) {
      const authorities = authoritiesForPath(item.href)
      assert.ok(
        authorities,
        `${item.href} must be covered by PORTAL_ROUTE_POLICY (coarse portal.read or the settings.read override)`,
      )
    }
  }
  // The one route-policy refinement is the settings subtree.
  assert.deepEqual(authoritiesForPath('/portal/settings'), ['settings.read'])
})
