// ---------------------------------------------------------------------------
// AUTH-01 — Portal authentication (targeted).
//
// SCOPED tests only: the changed seams (Auth.js session adapter, break-glass
// Credentials integration, canonical provider-subject resolution) and directly
// adjacent security layers (break-glass verification, audit write). No full
// harness, no unrelated regression.
//
// Pure tests (no DB): session-adapter shape mapping, JWT expiry config,
// break-glass secret verification.
// Persistence tests (DEV Neon branch via db/client): matrix A–O against the
// canonical auth_identity → app_user → role/authority projection, using
// tunit- prefixed fixtures that are removed in after(). The AUTH-01 dev
// bootstrap (owner role + break-glass identity on lisa@culebraluxe.com) must
// be applied first — see docs/auth-bootstrap-order.md.
// ---------------------------------------------------------------------------

import { test, before, after } from 'node:test'
import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'

import { sql } from '../../../db/client'
import { hashBreakGlassSecret, verifyBreakGlassSecret } from '../../../lib/auth/break-glass-secret'
import { authenticateBreakGlass } from '../../../lib/auth/break-glass-authenticate'
import { getActingUser } from '../../../lib/auth/get-acting-user'
import { createAuthJsSessionAdapter } from '../../../lib/auth/authjs-session-adapter'
import { UnauthenticatedError, UnmappedIdentityError, InactiveAccountError } from '../../../lib/auth/errors'
import { recordSecurityAuditEvent } from '../../../db/security-audit'
import { breakGlassSubject, SESSION_MAX_AGE_SECONDS } from '../../../auth'
import type { AuthenticatedIdentity } from '../../../lib/auth/types'
import type { SessionAdapter } from '../../../lib/auth/session-adapter'

// ---------------------------------------------------------------------------
// DEV bootstrap constants (docs/auth-bootstrap-order.md)
// ---------------------------------------------------------------------------

// lisa@culebraluxe.com — the bootstrapped internal owner in the DEV database.
const ROOT_USER_ID = 'aa06d089-162c-4bef-84ec-a76ee38cc8ad'
const ROOT_BREAK_GLASS_SUBJECT = breakGlassSubject(ROOT_USER_ID)

// Seeded role → authority sets (migration 015). Exact sets — no wildcard.
const OWNER_AUTHORITIES = [
  'crm.write',
  'deal.read',
  'deal.write',
  'listing.write',
  'portal.read',
  'settings.manage',
  'settings.read',
]
const AGENT_AUTHORITIES = [
  'crm.write',
  'deal.read',
  'deal.write',
  'listing.write',
  'portal.read',
]
const VIEWER_AUTHORITIES = ['deal.read', 'portal.read']
const CLIENT_AUTHORITIES = ['external.deal.read_own', 'external.properties.save']

// Env keys the break-glass tests override (saved/restored per test).
const BREAK_GLASS_ENV_KEYS = [
  'AUTH_BREAK_GLASS_ENABLED',
  'AUTH_BREAK_GLASS_APP_USER_ID',
  'AUTH_BREAK_GLASS_SECRET_HASH',
] as const

async function withBreakGlassEnv<T>(
  patch: Record<string, string | undefined>,
  fn: () => T | Promise<T>,
): Promise<T> {
  const saved = new Map<string, string | undefined>()
  for (const key of BREAK_GLASS_ENV_KEYS) {
    saved.set(key, process.env[key])
    delete process.env[key]
  }
  for (const [key, value] of Object.entries(patch)) {
    if (value === undefined) delete process.env[key]
    else process.env[key] = value
  }
  try {
    return await fn()
  } finally {
    for (const [key, value] of saved) {
      if (value === undefined) delete process.env[key]
      else process.env[key] = value
    }
  }
}

// ---------------------------------------------------------------------------
// Fixtures (tunit- prefixed; removed in after())
// ---------------------------------------------------------------------------

type FixtureUser = {
  id: string
  subject: string
}

const fixtures: FixtureUser[] = []

async function createFixtureUser(opts: {
  displayName: string
  accountType: 'internal' | 'external'
  active: boolean
  roleCode: string | null
}): Promise<FixtureUser> {
  const email = `tunit-auth-${randomUUID()}@test.local`
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

function stubAdapter(session: AuthenticatedIdentity | null): SessionAdapter {
  return { getSession: async () => session }
}

let agent: FixtureUser
let viewer: FixtureUser
let client: FixtureUser
let inactiveUser: FixtureUser

before(async () => {
  agent = await createFixtureUser({ displayName: 'Agent', accountType: 'internal', active: true, roleCode: 'agent' })
  viewer = await createFixtureUser({ displayName: 'Viewer', accountType: 'internal', active: true, roleCode: 'viewer' })
  client = await createFixtureUser({ displayName: 'Client', accountType: 'external', active: true, roleCode: 'client' })
  inactiveUser = await createFixtureUser({ displayName: 'Inactive', accountType: 'internal', active: false, roleCode: 'viewer' })
})

after(async () => {
  await sql`delete from security_audit_event where event_type = 'TEST_AUTH01_AUDIT'`
  await sql`delete from auth_identity where provider_subject like 'tunit-%'`
  await sql`
    delete from app_user_role
    where app_user_id in (select id from app_user where email like 'tunit-auth-%@test.local')
  `
  await sql`delete from app_user where email like 'tunit-auth-%@test.local'`
})

// ---------------------------------------------------------------------------
// Pure — adapter shape (criterion 2)
// ---------------------------------------------------------------------------

test('AUTH-01: adapter maps an Auth.js session to the provider identity (subject is the key, never email)', async () => {
  const fakeAuth = async () => ({
    expires: '2099-01-01T00:00:00.000Z',
    user: { name: 'Lisa', email: 'lisa@culebraluxe.com', image: null, sub: 'google-sub-123', provider: 'google' },
  })
  const adapter = createAuthJsSessionAdapter({ auth: fakeAuth as never })
  const identity = await adapter.getSession()
  assert.deepEqual(identity, {
    provider: 'google',
    providerSubject: 'google-sub-123',
    providerEmail: 'lisa@culebraluxe.com',
  })
})

test('AUTH-01: adapter returns null when no session or no stable subject exists', async () => {
  const noSession = createAuthJsSessionAdapter({ auth: (async () => null) as never })
  assert.equal(await noSession.getSession(), null)

  const noSub = createAuthJsSessionAdapter({
    auth: (async () => ({ expires: 'x', user: { name: null, email: null, image: null, sub: null, provider: 'google' } })) as never,
  })
  assert.equal(await noSub.getSession(), null)
})

test('AUTH-01: JWT session expiry is configured (logout clears the cookie via /api/auth/signout)', () => {
  assert.ok(SESSION_MAX_AGE_SECONDS >= 60 * 60 * 24, 'session must be configured with a non-trivial maxAge')
})

// ---------------------------------------------------------------------------
// Pure — break-glass secret verification (matrix H/I primitive)
// ---------------------------------------------------------------------------

test('AUTH-01: break-glass secret verification is symmetric and rejects wrong/malformed secrets', () => {
  const hash = hashBreakGlassSecret('correct horse battery staple')
  assert.equal(verifyBreakGlassSecret('correct horse battery staple', hash), true)
  assert.equal(verifyBreakGlassSecret('wrong', hash), false)
  assert.equal(verifyBreakGlassSecret('', hash), false)
  assert.equal(verifyBreakGlassSecret('x', 'not-a-scrypt-hash'), false)
})

// ---------------------------------------------------------------------------
// Matrix A — unauthenticated
// ---------------------------------------------------------------------------

test('AUTH-01 (A): unauthenticated session is rejected with UnauthenticatedError', async () => {
  await assert.rejects(() => getActingUser(stubAdapter(null)), (error: unknown) => {
    assert.ok(error instanceof UnauthenticatedError)
    assert.equal((error as UnauthenticatedError).code, 'unauthenticated')
    return true
  })
})

// ---------------------------------------------------------------------------
// Matrix B — unknown provider subject: no account creation, no email fallback
// ---------------------------------------------------------------------------

test('AUTH-01 (B): unknown provider subject is rejected and NO identity or account is created', async () => {
  const unknownSubject = `tunit-google-unknown-${randomUUID()}`
  const unknownEmail = `tunit-never-created-${randomUUID()}@test.local`
  await assert.rejects(
    () =>
      getActingUser(
        stubAdapter({
          provider: 'google',
          providerSubject: unknownSubject,
          providerEmail: unknownEmail,
        }),
      ),
    (error: unknown) => {
      assert.ok(error instanceof UnmappedIdentityError)
      assert.equal((error as UnmappedIdentityError).code, 'unmapped-identity')
      return true
    },
  )

  const identityRows = await sql`
    select 1 from auth_identity
    where provider = 'google' and provider_subject = ${unknownSubject}
  `
  assert.equal(identityRows.length, 0, 'no auth_identity row may be auto-created')
  const userRows = await sql`
    select 1 from app_user where email = ${unknownEmail}
  `
  assert.equal(userRows.length, 0, 'no app_user may be auto-created')
})

// ---------------------------------------------------------------------------
// Matrix C — mapped inactive app_user
// ---------------------------------------------------------------------------

test('AUTH-01 (C): mapped inactive app_user is rejected with InactiveAccountError', async () => {
  await assert.rejects(
    () => getActingUser(stubAdapter({ provider: 'google', providerSubject: inactiveUser.subject, providerEmail: null })),
    (error: unknown) => {
      assert.ok(error instanceof InactiveAccountError)
      assert.equal((error as InactiveAccountError).code, 'inactive-account')
      return true
    },
  )
})

// ---------------------------------------------------------------------------
// Matrices D/E/F/G — canonical resolution of mapped users
// ---------------------------------------------------------------------------

test('AUTH-01 (D): mapped active agent resolves the exact agent authority set', async () => {
  const actingUser = await getActingUser(stubAdapter({ provider: 'google', providerSubject: agent.subject, providerEmail: null }))
  assert.equal(actingUser.appUserId, agent.id)
  assert.equal(actingUser.accountType, 'internal')
  assert.deepEqual(actingUser.authorityCodes, AGENT_AUTHORITIES)
})

test('AUTH-01 (E): mapped viewer resolves the exact viewer authority set', async () => {
  const actingUser = await getActingUser(stubAdapter({ provider: 'google', providerSubject: viewer.subject, providerEmail: null }))
  assert.equal(actingUser.appUserId, viewer.id)
  assert.deepEqual(actingUser.authorityCodes, VIEWER_AUTHORITIES)
})

test('AUTH-01 (F): external client authenticates but gains no portal.read', async () => {
  const actingUser = await getActingUser(stubAdapter({ provider: 'google', providerSubject: client.subject, providerEmail: null }))
  assert.equal(actingUser.accountType, 'external')
  assert.deepEqual(actingUser.authorityCodes, CLIENT_AUTHORITIES)
  assert.ok(!actingUser.authorityCodes.includes('portal.read'), 'external client must not gain portal.read')
})

test('AUTH-01 (G/O): owner resolves the exact explicit owner authorities — no wildcard, no extras', async () => {
  // Temporary second identity for the root owner (matrix R: one user, many identities).
  const tempGoogleSubject = `tunit-google-owner-${randomUUID()}`
  await sql`
    insert into auth_identity (app_user_id, provider, provider_subject, provider_email)
    values (${ROOT_USER_ID}, 'google', ${tempGoogleSubject}, null)
  `
  try {
    const actingUser = await getActingUser(stubAdapter({ provider: 'google', providerSubject: tempGoogleSubject, providerEmail: null }))
    assert.equal(actingUser.appUserId, ROOT_USER_ID)
    assert.deepEqual(actingUser.authorityCodes, OWNER_AUTHORITIES)
    assert.ok(!actingUser.authorityCodes.includes('*'), 'no wildcard authority may exist')

    // Both identities coexist on one app_user (matrix R).
    const identities = await sql`
      select provider from auth_identity where app_user_id = ${ROOT_USER_ID}
    `
    const providers = (identities as { provider: string }[]).map((r) => r.provider)
    assert.ok(providers.includes('break-glass'))
    assert.ok(providers.includes('google'))
  } finally {
    await sql`delete from auth_identity where provider_subject = ${tempGoogleSubject}`
  }
})

test('AUTH-01 (P): provider email changes do not affect resolution — subject is the identity key', async () => {
  const actingUser = await getActingUser(
    stubAdapter({ provider: 'google', providerSubject: agent.subject, providerEmail: 'changed-email@example.com' }),
  )
  assert.equal(actingUser.appUserId, agent.id)
})

// ---------------------------------------------------------------------------
// Matrix Q — duplicate (provider, subject) is structurally impossible
// ---------------------------------------------------------------------------

test('AUTH-01 (Q): the same provider subject cannot be mapped twice (UNIQUE)', async () => {
  await assert.rejects(
    () =>
      sql`
        insert into auth_identity (app_user_id, provider, provider_subject, provider_email)
        values (${viewer.id}, 'google', ${agent.subject}, null)
      `,
    (error: unknown) => {
      assert.match(String((error as Error).message), /auth_identity_provider_subject_unique/i)
      return true
    },
  )
})

// ---------------------------------------------------------------------------
// Break-glass matrices H–O
// ---------------------------------------------------------------------------

const MATRIX_N_SECRET = 'auth-01-matrix-n-secret'

test('AUTH-01 (H): disabled break-glass fails closed', async () => {
  const result = await withBreakGlassEnv(
    { AUTH_BREAK_GLASS_ENABLED: 'false', AUTH_BREAK_GLASS_APP_USER_ID: ROOT_USER_ID, AUTH_BREAK_GLASS_SECRET_HASH: hashBreakGlassSecret(MATRIX_N_SECRET) },
    () => authenticateBreakGlass(MATRIX_N_SECRET),
  )
  assert.deepEqual(result, { ok: false, reason: 'disabled' })
})

test('AUTH-01 (I): wrong break-glass secret fails generically', async () => {
  const result = await withBreakGlassEnv(
    {
      AUTH_BREAK_GLASS_ENABLED: 'true',
      AUTH_BREAK_GLASS_APP_USER_ID: ROOT_USER_ID,
      AUTH_BREAK_GLASS_SECRET_HASH: hashBreakGlassSecret(MATRIX_N_SECRET),
    },
    () => authenticateBreakGlass('definitely-wrong-secret'),
  )
  assert.deepEqual(result, { ok: false, reason: 'invalid' })
})

test('AUTH-01 (J): break-glass configured user nonexistent fails as unavailable', async () => {
  const result = await withBreakGlassEnv(
    {
      AUTH_BREAK_GLASS_ENABLED: 'true',
      AUTH_BREAK_GLASS_APP_USER_ID: randomUUID(),
      AUTH_BREAK_GLASS_SECRET_HASH: hashBreakGlassSecret(MATRIX_N_SECRET),
    },
    () => authenticateBreakGlass(MATRIX_N_SECRET),
  )
  assert.deepEqual(result, { ok: false, reason: 'unavailable' })
})

test('AUTH-01 (K): break-glass configured user inactive fails as unavailable', async () => {
  const result = await withBreakGlassEnv(
    {
      AUTH_BREAK_GLASS_ENABLED: 'true',
      AUTH_BREAK_GLASS_APP_USER_ID: inactiveUser.id,
      AUTH_BREAK_GLASS_SECRET_HASH: hashBreakGlassSecret(MATRIX_N_SECRET),
    },
    () => authenticateBreakGlass(MATRIX_N_SECRET),
  )
  assert.deepEqual(result, { ok: false, reason: 'unavailable' })
})

test('AUTH-01 (L): break-glass configured external user fails as not-owner', async () => {
  const result = await withBreakGlassEnv(
    {
      AUTH_BREAK_GLASS_ENABLED: 'true',
      AUTH_BREAK_GLASS_APP_USER_ID: client.id,
      AUTH_BREAK_GLASS_SECRET_HASH: hashBreakGlassSecret(MATRIX_N_SECRET),
    },
    () => authenticateBreakGlass(MATRIX_N_SECRET),
  )
  assert.deepEqual(result, { ok: false, reason: 'not-owner' })
})

test('AUTH-01 (M): break-glass configured internal user without owner role fails as not-owner', async () => {
  const result = await withBreakGlassEnv(
    {
      AUTH_BREAK_GLASS_ENABLED: 'true',
      AUTH_BREAK_GLASS_APP_USER_ID: agent.id,
      AUTH_BREAK_GLASS_SECRET_HASH: hashBreakGlassSecret(MATRIX_N_SECRET),
    },
    () => authenticateBreakGlass(MATRIX_N_SECRET),
  )
  assert.deepEqual(result, { ok: false, reason: 'not-owner' })
})

test('AUTH-01 (N/O): valid break-glass root resolves the same canonical ActingUser model, no wildcard', async () => {
  const result = await withBreakGlassEnv(
    {
      AUTH_BREAK_GLASS_ENABLED: 'true',
      AUTH_BREAK_GLASS_APP_USER_ID: ROOT_USER_ID,
      AUTH_BREAK_GLASS_SECRET_HASH: hashBreakGlassSecret(MATRIX_N_SECRET),
    },
    () => authenticateBreakGlass(MATRIX_N_SECRET),
  )
  assert.ok(result.ok)
  if (!result.ok) return
  assert.equal(result.actingUser.appUserId, ROOT_USER_ID)
  assert.equal(result.actingUser.accountType, 'internal')
  assert.ok(result.actingUser.roleCodes.includes('owner'))
  assert.deepEqual(result.actingUser.authorityCodes, OWNER_AUTHORITIES)
  assert.ok(!result.actingUser.authorityCodes.includes('*'), 'no wildcard authority may exist')
})

test('AUTH-01 (N via adapter): a break-glass session resolves through the full adapter → getActingUser pipeline', async () => {
  // Simulates the Auth.js Credentials session the recovery action establishes:
  // provider = 'break-glass', subject = breakGlassSubject(root app_user).
  const fakeAuth = async () => ({
    expires: '2099-01-01T00:00:00.000Z',
    user: { name: 'Lisa Penfield', email: null, image: null, sub: ROOT_BREAK_GLASS_SUBJECT, provider: 'break-glass' },
  })
  const adapter = createAuthJsSessionAdapter({ auth: fakeAuth as never })
  const identity = await adapter.getSession()
  assert.deepEqual(identity, { provider: 'break-glass', providerSubject: ROOT_BREAK_GLASS_SUBJECT, providerEmail: null })

  const actingUser = await getActingUser(adapter)
  assert.equal(actingUser.appUserId, ROOT_USER_ID)
  assert.deepEqual(actingUser.authorityCodes, OWNER_AUTHORITIES)
})

// ---------------------------------------------------------------------------
// Audit write (criterion 5)
// ---------------------------------------------------------------------------

test('AUTH-01: a successful break-glass login is audited', async () => {
  await recordSecurityAuditEvent({
    appUserId: ROOT_USER_ID,
    eventType: 'TEST_AUTH01_AUDIT',
    authenticationMethod: 'break-glass',
    metadata: { accountType: 'internal', fixture: true },
  })
  const rows = await sql`
    select event_type, authentication_method, app_user_id
    from security_audit_event
    where event_type = 'TEST_AUTH01_AUDIT'
  `
  assert.equal(rows.length, 1)
  const row = rows[0] as { event_type: string; authentication_method: string; app_user_id: string }
  assert.equal(row.event_type, 'TEST_AUTH01_AUDIT')
  assert.equal(row.authentication_method, 'break-glass')
  assert.equal(row.app_user_id, ROOT_USER_ID)
})
