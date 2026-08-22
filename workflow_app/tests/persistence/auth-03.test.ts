// ---------------------------------------------------------------------------
// AUTH-03 — Server Action Authorization (targeted).
//
// SCOPED tests only: the changed seams (runAuthorized enforcement on every
// Portal write, guardPortalUpload on both media upload routes, the
// portalWrite wiring in app/portal/actions.ts) and directly adjacent security
// layers. No full harness, no unrelated regression.
//
// The crux of the story: client-side button hiding is never sufficient. These
// tests prove an unauthenticated caller, an authenticated viewer (no write
// authority), and an external client cannot mutate — the underlying business
// service / SQL write never runs.
//
// Pure tests (no DB): runAuthorized denial matrix with stub adapters + fake
// handlers, guardPortalUpload 401/403 mapping, and the action↔authority wiring
// contract parsed from docs/auth-command-map.md + app/portal/actions.ts.
// Persistence tests (DEV Neon branch via db/client): the canonical actor
// projections (viewer / external client / owner) used by the denial matrix,
// plus no-mutation proofs through the REAL server actions and REAL upload
// route handlers — outside a request scope no session can exist, so every
// authenticated write must fail closed before its SQL runs.
// ---------------------------------------------------------------------------

import { test, before, after } from 'node:test'
import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'
import { readFileSync } from 'node:fs'

import { sql } from '../../../db/client'
import { runAuthorized } from '../../../lib/auth/require-authority'
import {
  getPortalActingUser,
  guardPortalUpload,
} from '../../../lib/auth/portal-session'
import {
  MissingAuthorityError,
  UnauthenticatedError,
} from '../../../lib/auth/errors'
import type { AuthenticatedIdentity } from '../../../lib/auth/types'
import type { SessionAdapter } from '../../../lib/auth/session-adapter'
import { breakGlassSubject } from '../../../auth'
import * as portalActions from '../../../app/portal/actions'
import { POST as mediaUploadPost } from '../../../app/api/media/upload/route'
import { POST as propertyMediaUploadPost } from '../../../app/api/property-media/upload/route'

// ---------------------------------------------------------------------------
// DEV bootstrap constants (docs/auth-bootstrap-order.md) + fixture users
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

type FixtureUser = { id: string; subject: string }

const fixtures: FixtureUser[] = []

async function createFixtureUser(opts: {
  displayName: string
  accountType: 'internal' | 'external'
  roleCode: string | null
}): Promise<FixtureUser> {
  const email = `tunit-auth03-${randomUUID()}@test.local`
  const subject = `tunit-google-${opts.displayName.toLowerCase().replace(/\s+/g, '-')}-${randomUUID()}`
  const rows = await sql`
    insert into app_user (display_name, email, account_type, active)
    values (${opts.displayName}, ${email}, ${opts.accountType}, true)
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

let viewer: FixtureUser
let client: FixtureUser
let owner: FixtureUser

before(async () => {
  viewer = await createFixtureUser({
    displayName: 'Viewer',
    accountType: 'internal',
    roleCode: 'viewer',
  })
  client = await createFixtureUser({
    displayName: 'Client',
    accountType: 'external',
    roleCode: 'client',
  })
  owner = await createFixtureUser({
    displayName: 'Owner',
    accountType: 'internal',
    roleCode: 'owner',
  })
})

after(async () => {
  // Defensive cleanup for the no-mutation proofs (they must not have written,
  // but if a regression lets a write through, the fixture rows are removed).
  await sql`delete from task where title like 'tunit-auth03-%'`
  await sql`delete from media where filename like 'tunit-auth03-%'`
  await sql`delete from auth_identity where provider_subject like 'tunit-%'`
  await sql`
    delete from app_user_role
    where app_user_id in (select id from app_user where email like 'tunit-auth03-%@test.local')
  `
  await sql`delete from app_user where email like 'tunit-auth03-%@test.local'`
})

function stubAdapter(session: AuthenticatedIdentity | null): SessionAdapter {
  return { getSession: async () => session }
}

// ---------------------------------------------------------------------------
// Pure — runAuthorized denial matrix (the reusable enforcement seam).
// A fake handler stands in for the business service: denial must throw BEFORE
// the handler runs, so the underlying write can never execute.
// ---------------------------------------------------------------------------

test('AUTH-03: unauthenticated caller is denied UnauthenticatedError and the handler never runs', async () => {
  let handlerCalled = false
  await assert.rejects(
    runAuthorized(stubAdapter(null), 'crm.write', async () => {
      handlerCalled = true
      return 'should-not-run'
    }),
    UnauthenticatedError,
  )
  assert.equal(handlerCalled, false, 'business service must never run on denial')
})

test('AUTH-03: authenticated viewer is denied every write authority (crm/deal/listing) with MissingAuthorityError and no mutation', async () => {
  const viewerAdapter = stubAdapter({
    provider: 'google',
    providerSubject: viewer.subject,
    providerEmail: null,
  })
  for (const authority of ['crm.write', 'deal.write', 'listing.write'] as const) {
    let handlerCalled = false
    await assert.rejects(
      runAuthorized(viewerAdapter, authority, async () => {
        handlerCalled = true
        return 'should-not-run'
      }),
      MissingAuthorityError,
      `viewer must be denied ${authority}`,
    )
    assert.equal(
      handlerCalled,
      false,
      `business service must never run for viewer on ${authority}`,
    )
  }
})

test('AUTH-03: authenticated external client is denied every write authority (crm/deal/listing) with MissingAuthorityError and no mutation', async () => {
  const clientAdapter = stubAdapter({
    provider: 'google',
    providerSubject: client.subject,
    providerEmail: null,
  })
  for (const authority of ['crm.write', 'deal.write', 'listing.write'] as const) {
    let handlerCalled = false
    await assert.rejects(
      runAuthorized(clientAdapter, authority, async () => {
        handlerCalled = true
        return 'should-not-run'
      }),
      MissingAuthorityError,
      `external client must be denied ${authority}`,
    )
    assert.equal(
      handlerCalled,
      false,
      `business service must never run for external client on ${authority}`,
    )
  }
})

test('AUTH-03: owner resolves every write authority and the handler runs with the actor (positive control)', async () => {
  const ownerAdapter = stubAdapter({
    provider: 'break-glass',
    providerSubject: ROOT_BREAK_GLASS_SUBJECT,
    providerEmail: null,
  })
  for (const authority of ['crm.write', 'deal.write', 'listing.write'] as const) {
    let handlerCalled = false
    const result = await runAuthorized(ownerAdapter, authority, async (actor) => {
      handlerCalled = true
      return actor.appUserId
    })
    assert.equal(handlerCalled, true, `handler must run for owner on ${authority}`)
    assert.equal(result, ROOT_USER_ID)
  }
})

// ---------------------------------------------------------------------------
// Pure — guardPortalUpload mapping for the media upload route handlers.
// ---------------------------------------------------------------------------

test('AUTH-03: upload guard returns 401 for an unauthenticated caller', async () => {
  const result = await guardPortalUpload('listing.write', stubAdapter(null))
  assert.deepEqual(result, { ok: false, status: 401, error: 'Unauthorized.' })
})

test('AUTH-03: upload guard returns 403 for a viewer and an external client (no listing.write)', async () => {
  const viewerResult = await guardPortalUpload(
    'listing.write',
    stubAdapter({ provider: 'google', providerSubject: viewer.subject, providerEmail: null }),
  )
  assert.deepEqual(viewerResult, { ok: false, status: 403, error: 'Unauthorized.' })

  const clientResult = await guardPortalUpload(
    'listing.write',
    stubAdapter({ provider: 'google', providerSubject: client.subject, providerEmail: null }),
  )
  assert.deepEqual(clientResult, { ok: false, status: 403, error: 'Unauthorized.' })
})

test('AUTH-03: upload guard admits an owner with listing.write and returns the actor', async () => {
  const ownerAdapter = stubAdapter({
    provider: 'break-glass',
    providerSubject: ROOT_BREAK_GLASS_SUBJECT,
    providerEmail: null,
  })
  const result = await guardPortalUpload('listing.write', ownerAdapter)
  assert.ok(result.ok)
  if (!result.ok) return
  assert.equal(result.actor.appUserId, ROOT_USER_ID)
})

test('AUTH-03: getPortalActingUser resolves the canonical actor through an injected adapter', async () => {
  const ownerAdapter = stubAdapter({
    provider: 'break-glass',
    providerSubject: ROOT_BREAK_GLASS_SUBJECT,
    providerEmail: null,
  })
  const actingUser = await getPortalActingUser(ownerAdapter)
  assert.equal(actingUser.appUserId, ROOT_USER_ID)
  assert.deepEqual(actingUser.authorityCodes, OWNER_AUTHORITIES)
})

// ---------------------------------------------------------------------------
// Pure — action ↔ authority wiring contract.
// Parses docs/auth-command-map.md (the contract) and app/portal/actions.ts
// (the implementation) and asserts every mapped write action is gated with the
// EXACT mapped authority via portalWrite, and no write action is left ungated.
// ---------------------------------------------------------------------------

const ROOT = new URL('../../../', import.meta.url)

function parseCommandMap(): Map<string, string> {
  const doc = readFileSync(new URL('docs/auth-command-map.md', ROOT), 'utf8')
  const lines = doc.split('\n')
  const inWrites =
    lines.findIndex((l) => l.startsWith('## Writes')) + 1
  const outOfWrites = lines.findIndex(
    (l, i) => i > inWrites && l.startsWith('## '),
  )
  const map = new Map<string, string>()
  for (const line of lines.slice(inWrites, outOfWrites)) {
    // Every row is "| `action(s)` | `authority` |" — the last code span is the
    // authority and the rest are action names. Span parsing (not cell
    // splitting) keeps rows like "`resolveIntakeAction` (attach | create |
    // reject)" correct despite the pipes in the prose.
    const spans = [...line.matchAll(/`([^`]+)`/g)].map((m) => m[1])
    if (spans.length < 2) continue
    const authority = spans[spans.length - 1]
    for (const name of spans.slice(0, -1)) map.set(name, authority)
  }
  return map
}

function parseActionGates(): Map<string, string> {
  const src = readFileSync(new URL('app/portal/actions.ts', ROOT), 'utf8')
  const lines = src.split('\n')
  const gates = new Map<string, string>()
  let currentAction: string | null = null
  for (const line of lines) {
    const actionMatch = line.match(/^export async function (\w+)\(/)
    if (actionMatch) {
      currentAction = actionMatch[1]
      continue
    }
    const gateMatch = line.match(/portalWrite\('([^']+)'/)
    if (gateMatch && currentAction) {
      gates.set(currentAction, gateMatch[1])
    }
  }
  return gates
}

test('AUTH-03: every write action in docs/auth-command-map.md is gated with the exact mapped authority', () => {
  const map = parseCommandMap()
  const gates = parseActionGates()
  assert.ok(map.size >= 24, `command map must list the Portal write inventory (found ${map.size})`)
  for (const [action, authority] of map) {
    assert.equal(
      gates.get(action),
      authority,
      `${action} must enforce ${authority} via portalWrite`,
    )
  }
})

test('AUTH-03: every write export of app/portal/actions.ts is gated and the read export is not', () => {
  const src = readFileSync(new URL('app/portal/actions.ts', ROOT), 'utf8')
  const exports_ = [...src.matchAll(/^export async function (\w+)\(/gm)].map((m) => m[1])
  const gates = parseActionGates()

  const writeExports = exports_.filter((name) => name !== 'searchPeopleAction')
  // AUTH-03 originally shipped 24 write actions; OPS-02 added the four client
  // administration actions and OPS-03 adds the three property lifecycle
  // actions (create / archive / restore), so the inventory is now 31. This
  // count exists to force an explicit review whenever the write surface grows.
  assert.equal(writeExports.length, 31, 'Portal write inventory must be exactly 31 actions')
  for (const name of writeExports) {
    assert.ok(
      gates.has(name),
      `${name} must be gated — client-side hiding is never sufficient`,
    )
  }
  assert.ok(!gates.has('searchPeopleAction'), 'read-only searchPeopleAction must not be treated as a write')
  assert.ok(
    [...gates.values()].every((a) => ['crm.write', 'deal.write', 'listing.write'].includes(a)),
    'every action authority must be one of crm.write / deal.write / listing.write',
  )
})

// ---------------------------------------------------------------------------
// Persistence — no mutation through the REAL server actions.
// Outside a request scope no session can exist, so an authenticated write must
// fail closed before its SQL runs. The exact UnauthenticatedError is proven at
// the runAuthorized seam above; here we prove the real action resolves the
// actor BEFORE any write and that the underlying row is never created.
// ---------------------------------------------------------------------------

test('AUTH-03: unauthenticated real createTaskAction fails closed and never writes a task', async () => {
  const personId = randomUUID()
  const title = `tunit-auth03-${randomUUID()}`
  await assert.rejects(
    portalActions.createTaskAction({ title, personId }),
    undefined,
    'outside a request scope no session exists, so the write must fail before running',
  )
  const rows = await sql`
    select count(*)::int as n from task
    where title = ${title} and person_id = ${personId}
  `
  assert.equal(rows[0].n, 0, 'no task row may be created by an unauthenticated caller')
})

test('AUTH-03: unauthenticated real completeTaskAction fails closed before completing', async () => {
  // A UUID that cannot exist as a task; the point is the auth resolution
  // happens before any write is attempted, so no error/write path is reached.
  const taskId = randomUUID()
  await assert.rejects(
    portalActions.completeTaskAction(taskId),
    undefined,
    'unauthenticated caller must be rejected before the db layer',
  )
})

// ---------------------------------------------------------------------------
// Persistence — no mutation through the REAL media upload route handlers.
// ---------------------------------------------------------------------------

test('AUTH-03: unauthenticated /api/media/upload fails closed and never inserts media', async () => {
  const filename = `tunit-auth03-${randomUUID()}.bin`
  const form = new FormData()
  form.append(
    'file',
    new File([new Uint8Array([1, 2, 3])], filename, {
      type: 'application/octet-stream',
    }),
  )
  const res = await mediaUploadPost(
    new Request('http://localhost/api/media/upload', { method: 'POST', body: form }),
  )
  assert.ok(res.status >= 400, `unauthenticated upload must fail closed (got ${res.status})`)
  const rows = await sql`
    select count(*)::int as n from media
    where filename = ${filename}
  `
  assert.equal(rows[0].n, 0, 'no media row may be inserted by an unauthenticated caller')
})

test('AUTH-03: unauthenticated /api/property-media/upload fails closed and never inserts media', async () => {
  const filename = `tunit-auth03-${randomUUID()}.png`
  const form = new FormData()
  form.append('propertyId', randomUUID())
  form.append('role', 'gallery')
  form.append('altText', 'tunit auth03 probe')
  form.append(
    'file',
    new File([new Uint8Array([1, 2, 3])], filename, { type: 'image/png' }),
  )
  const res = await propertyMediaUploadPost(
    new Request('http://localhost/api/property-media/upload', {
      method: 'POST',
      body: form,
    }),
  )
  assert.ok(res.status >= 400, `unauthenticated upload must fail closed (got ${res.status})`)
  const rows = await sql`
    select count(*)::int as n from media
    where filename = ${filename}
  `
  assert.equal(rows[0].n, 0, 'no media row may be inserted by an unauthenticated caller')
})
