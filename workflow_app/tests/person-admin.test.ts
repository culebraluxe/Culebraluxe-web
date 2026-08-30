import { test } from 'node:test'
import assert from 'node:assert/strict'

import { PortalWriteError } from '../../lib/portal-write-error'
import {
  CLIENT_ROLES,
  CLIENT_STATUSES,
  normalizeClientContact,
  normalizeClientCreateInput,
  normalizeClientProfileUpdate,
} from '../../lib/person-admin'
import {
  archiveClient,
  createClient,
  setClientIdentity,
  updateClientProfile,
} from '../../db/person-admin'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// OPS-02 — Client Administration: targeted unit suite for the client CRUD
// seam. The pure contract (lib/person-admin.ts) is tested directly; the SQL
// seam (db/person-admin.ts) is exercised through an injected in-memory fake
// TxRunner that models the person / person_identity / app_user tables. No
// database is touched (the imported db/client.ts is lazy and never queried).
// Real-Postgres coverage of these seams lives in the persistence suite.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

const UUID = '00000000-0000-4000-8000-000000000001'

class FakePersonAdmin {
  persons: Row[] = []
  identities: Row[] = []
  users: Row[] = []

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''),
        '',
      ),
    )
    const p = params as any[]

    // person row read (personRow)
    if (t.startsWith('select id, display_name, role, status, location, budget_min')) {
      const row = this.persons.find((r) => r.id === p[0])
      return Promise.resolve(row ? [{ ...row }] : [])
    }
    // assignable agent existence
    if (t.startsWith('select id from app_user')) {
      const row = this.users.find((r) => r.id === p[0] && r.active)
      return Promise.resolve(row ? [{ id: row.id }] : [])
    }
    // findIdentityOwnership
    if (t.includes('from person_identity') && t.includes('join person')) {
      const row = this.identities.find(
        (r) => r.identity_type === p[0] && r.identity_value === p[1],
      )
      if (!row) return Promise.resolve([])
      const person = this.persons.find((r) => r.id === row.person_id)
      return Promise.resolve([
        {
          identity_id: row.id,
          person_id: row.person_id,
          identity_value: row.identity_value,
          archived_at: person?.archived_at ?? null,
        },
      ])
    }
    // insert into person
    if (t.startsWith('insert into person (')) {
      this.persons.push({
        id: p[0],
        display_name: p[1],
        role: p[2],
        status: p[3],
        location: p[4],
        budget_min: p[5],
        budget_max: p[6],
        preferred_areas: p[7],
        property_types: p[8],
        priorities: p[9],
        timeline: p[10],
        notes: p[11],
        assigned_user_id: p[12],
        archived_at: null,
      })
      return Promise.resolve([])
    }
    // insert into person_identity. 'email'/'phone' appear as SQL literals in
    // the create path and as a parameter in setClientIdentity; 'true' primary
    // is always literal. Detect the kind from the SQL text when possible.
    if (t.startsWith('insert into person_identity (')) {
      const hasLiteralKind = t.includes("'email'") || t.includes("'phone'")
      this.identities.push({
        id: `identity-${this.identities.length + 1}`,
        person_id: p[0],
        identity_type: hasLiteralKind
          ? t.includes("'email'")
            ? 'email'
            : 'phone'
          : p[1],
        identity_value: hasLiteralKind ? p[1] : p[2],
        is_primary: true,
      })
      return Promise.resolve([])
    }
    // update person profile (merged fixed SET)
    if (t.startsWith('update person set display_name')) {
      const person = this.persons.find(
        (r) => r.id === p[12] && r.archived_at === null,
      )
      if (!person) return Promise.resolve([])
      person.display_name = p[0]
      person.role = p[1]
      person.status = p[2]
      person.location = p[3]
      person.budget_min = p[4]
      person.budget_max = p[5]
      person.preferred_areas = p[6]
      person.property_types = p[7]
      person.priorities = p[8]
      person.timeline = p[9]
      person.notes = p[10]
      person.assigned_user_id = p[11]
      return Promise.resolve([{ id: person.id }])
    }
    // archive (soft delete)
    if (t.startsWith('update person set archived_at')) {
      const person = this.persons.find(
        (r) => r.id === p[0] && r.archived_at === null,
      )
      if (!person) return Promise.resolve([])
      person.archived_at = new Date().toISOString()
      return Promise.resolve([{ id: person.id }])
    }
    // unset primary for kind on person
    if (t.startsWith('update person_identity set is_primary = false')) {
      for (const row of this.identities) {
        if (row.person_id === p[0] && row.identity_type === p[1]) {
          row.is_primary = false
        }
      }
      return Promise.resolve([])
    }
    // set primary by identity id
    if (t.startsWith('update person_identity set is_primary = true')) {
      const row = this.identities.find((r) => r.id === p[0])
      if (row) row.is_primary = true
      return Promise.resolve([])
    }
    // delete identities of a kind
    if (t.startsWith('delete from person_identity')) {
      this.identities = this.identities.filter(
        (r) => !(r.person_id === p[0] && r.identity_type === p[1]),
      )
      return Promise.resolve([])
    }

    throw new Error(`FAKE_PERSON_ADMIN_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

function seededFake() {
  const fake = new FakePersonAdmin()
  fake.users.push({ id: 'user-1', display_name: 'Lisa', active: true })
  fake.persons.push({
    id: UUID,
    display_name: 'Existing Client',
    role: 'buyer',
    status: 'active',
    location: 'Culebra',
    budget_min: null,
    budget_max: null,
    preferred_areas: null,
    property_types: null,
    priorities: null,
    timeline: null,
    notes: null,
    assigned_user_id: null,
    archived_at: null,
  })
  return fake
}

// ---------------------------------------------------------------------------
// Pure contract — lib/person-admin.ts
// ---------------------------------------------------------------------------

test('normalizeClientCreateInput normalizes a full valid input', () => {
  const input = normalizeClientCreateInput({
    displayName: '  Ana  Rivera ',
    role: 'both',
    status: 'warm',
    location: ' Culebra ',
    email: ' ANA@Example.COM ',
    phone: '+1 (787) 555-0142',
    budgetMin: 500000,
    budgetMax: 800000,
    preferredAreas: [' Flamenco ', ''],
    timeline: ' 6 months ',
    notes: '  Ocean view  ',
  })

  assert.equal(input.displayName, 'Ana Rivera')
  assert.equal(input.role, 'both')
  assert.equal(input.status, 'warm')
  assert.equal(input.location, 'Culebra')
  assert.equal(input.email, 'ana@example.com')
  assert.equal(input.phone, '+17875550142')
  assert.equal(input.budgetMin, 500000)
  assert.equal(input.budgetMax, 800000)
  assert.deepEqual(input.preferredAreas, ['Flamenco'])
  assert.equal(input.timeline, '6 months')
  assert.equal(input.notes, 'Ocean view')
  assert.equal(input.assignedUserId, null)
})

test('normalizeClientCreateInput defaults status to new and contact to null', () => {
  const input = normalizeClientCreateInput({
    displayName: 'No Contact',
    role: 'seller',
  })
  assert.equal(input.status, 'new')
  assert.equal(input.email, null)
  assert.equal(input.phone, null)
})

test('normalizeClientCreateInput rejects an invalid role', () => {
  assert.throws(
    () =>
      normalizeClientCreateInput({
        displayName: 'X',
        role: 'developer' as any,
      }),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'validation' &&
      /role/i.test(error.message),
  )
})

test('normalizeClientCreateInput rejects a missing display name', () => {
  assert.throws(
    () =>
      normalizeClientCreateInput({
        displayName: '   ',
        role: 'buyer',
      }),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'validation' &&
      /name/i.test(error.message),
  )
})

test('normalizeClientCreateInput rejects an invalid email', () => {
  assert.throws(
    () =>
      normalizeClientCreateInput({
        displayName: 'X',
        role: 'buyer',
        email: 'not-an-email',
      }),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'validation' &&
      /email/i.test(error.message),
  )
})

test('normalizeClientCreateInput rejects a phone without a country code', () => {
  assert.throws(
    () =>
      normalizeClientCreateInput({
        displayName: 'X',
        role: 'buyer',
        phone: '7875550142',
      }),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'validation' &&
      /country code/i.test(error.message),
  )
})

test('normalizeClientCreateInput rejects min budget above max budget', () => {
  assert.throws(
    () =>
      normalizeClientCreateInput({
        displayName: 'X',
        role: 'buyer',
        budgetMin: 900000,
        budgetMax: 500000,
      }),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'validation' &&
      /budget/i.test(error.message),
  )
})

test('normalizeClientProfileUpdate keeps undefined keys, clears null keys', () => {
  const update = normalizeClientProfileUpdate({
    displayName: 'Renamed',
    location: null,
    preferredAreas: [' ', ''],
  })
  assert.equal(update.displayName, 'Renamed')
  assert.equal(update.location, null)
  assert.equal(update.preferredAreas, null)
  assert.equal('role' in update, false)
  assert.equal('notes' in update, false)
})

test('normalizeClientContact normalizes email and phone, clears empty', () => {
  assert.equal(normalizeClientContact('email', ' A@B.COM '), 'a@b.com')
  assert.equal(normalizeClientContact('phone', '+1 787 555 0142'), '+17875550142')
  assert.equal(normalizeClientContact('email', ''), null)
  assert.equal(normalizeClientContact('phone', null), null)
  assert.throws(
    () => normalizeClientContact('phone', '7875550142'),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'validation',
  )
})

test('closed role and status vocabularies match the person schema', () => {
  assert.deepEqual([...CLIENT_ROLES], ['buyer', 'seller', 'both', 'unclassified'])
  assert.deepEqual([...CLIENT_STATUSES], ['new', 'warm', 'active', 'referral'])
})

// ---------------------------------------------------------------------------
// DB seam — db/person-admin.ts through the fake TxRunner
// ---------------------------------------------------------------------------

test('createClient inserts the person and both identities atomically', async () => {
  const fake = seededFake()
  const before = fake.identities.length

  const result = await createClient(
    {
      displayName: 'New Buyer',
      role: 'buyer',
      email: 'new@example.com',
      phone: '+17875550142',
      assignedUserId: 'user-1',
    },
    fake.runner,
  )

  assert.ok(result.personId)
  assert.equal(fake.persons.length, 2)
  assert.equal(fake.identities.length, before + 2)
  const created = fake.persons.find((p) => p.id === result.personId)!
  assert.equal(created.display_name, 'New Buyer')
  assert.equal(created.status, 'new')
  assert.equal(created.assigned_user_id, 'user-1')
  assert.ok(
    fake.identities.some(
      (i) => i.person_id === result.personId && i.identity_type === 'email' && i.is_primary,
    ),
  )
})

test('createClient with no contact identities creates the person only', async () => {
  const fake = seededFake()
  const before = fake.identities.length

  const result = await createClient(
    { displayName: 'No Contact', role: 'seller' },
    fake.runner,
  )

  assert.ok(result.personId)
  assert.equal(fake.identities.length, before)
  assert.equal(fake.persons.length, 2)
})

test('createClient refuses an email already owned by another person', async () => {
  const fake = seededFake()
  fake.identities.push({
    id: 'identity-owned',
    person_id: UUID,
    identity_type: 'email',
    identity_value: 'taken@example.com',
    is_primary: true,
  })

  await assert.rejects(
    createClient(
      { displayName: 'Clash', role: 'buyer', email: 'taken@example.com' },
      fake.runner,
    ),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'conflict' &&
      /Existing Client/.test(error.message),
  )
  // No person was created on conflict (seeded roster unchanged).
  assert.equal(fake.persons.length, 1)
})

test('updateClientProfile merges provided fields and keeps the rest', async () => {
  const fake = seededFake()

  await updateClientProfile(
    UUID,
    { displayName: 'Renamed Client', status: 'warm', location: null },
    fake.runner,
  )

  const person = fake.persons.find((p) => p.id === UUID)!
  assert.equal(person.display_name, 'Renamed Client')
  assert.equal(person.status, 'warm')
  assert.equal(person.location, null)
  assert.equal(person.role, 'buyer', 'role left untouched')
  assert.equal(person.notes, null, 'notes left untouched')
})

test('updateClientProfile is not-found for a missing person', async () => {
  const fake = seededFake()
  await assert.rejects(
    updateClientProfile(
      '00000000-0000-4000-8000-0000000000ff',
      { displayName: 'Ghost' },
      fake.runner,
    ),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'not-found',
  )
})

test('updateClientProfile rejects a min budget above max budget', async () => {
  const fake = seededFake()
  await assert.rejects(
    updateClientProfile(UUID, { budgetMin: 900000, budgetMax: 500000 }, fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'validation',
  )
})

test('archiveClient soft-deletes and is a conflict on the second call', async () => {
  const fake = seededFake()

  await archiveClient(UUID, fake.runner)
  assert.ok(fake.persons.find((p) => p.id === UUID)!.archived_at)

  await assert.rejects(
    archiveClient(UUID, fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'conflict',
  )
})

test('archiveClient is not-found for a missing person', async () => {
  const fake = seededFake()
  await assert.rejects(
    archiveClient('00000000-0000-4000-8000-0000000000ff', fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'not-found',
  )
})

test('setClientIdentity inserts a new primary email and unmarks the old one', async () => {
  const fake = seededFake()
  fake.identities.push({
    id: 'identity-old',
    person_id: UUID,
    identity_type: 'email',
    identity_value: 'old@example.com',
    is_primary: true,
  })

  await setClientIdentity(UUID, 'email', 'NEW@example.com', fake.runner)

  const identities = fake.identities.filter(
    (i) => i.person_id === UUID && i.identity_type === 'email',
  )
  assert.equal(identities.length, 2)
  assert.equal(
    identities.find((i) => i.identity_value === 'new@example.com')?.is_primary,
    true,
  )
  assert.equal(
    identities.find((i) => i.identity_value === 'old@example.com')?.is_primary,
    false,
  )
})

test('setClientIdentity sets primary on an identity already owned by the person', async () => {
  const fake = seededFake()
  fake.identities.push(
    { id: 'identity-a', person_id: UUID, identity_type: 'phone', identity_value: '+17875550111', is_primary: true },
    { id: 'identity-b', person_id: UUID, identity_type: 'phone', identity_value: '+17875550142', is_primary: false },
  )

  await setClientIdentity(UUID, 'phone', '+17875550142', fake.runner)

  assert.equal(
    fake.identities.find((i) => i.id === 'identity-b')?.is_primary,
    true,
  )
  assert.equal(
    fake.identities.find((i) => i.id === 'identity-a')?.is_primary,
    false,
  )
})

test('setClientIdentity refuses a value owned by another person', async () => {
  const fake = seededFake()
  fake.identities.push({
    id: 'identity-other',
    person_id: '00000000-0000-4000-8000-000000000002',
    identity_type: 'email',
    identity_value: 'taken@example.com',
    is_primary: true,
  })

  await assert.rejects(
    setClientIdentity(UUID, 'email', 'taken@example.com', fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'conflict',
  )
})

test('setClientIdentity with null clears every identity of that kind', async () => {
  const fake = seededFake()
  fake.identities.push(
    { id: 'identity-a', person_id: UUID, identity_type: 'email', identity_value: 'a@example.com', is_primary: true },
    { id: 'identity-b', person_id: UUID, identity_type: 'phone', identity_value: '+17875550142', is_primary: true },
  )

  await setClientIdentity(UUID, 'email', null, fake.runner)

  assert.equal(
    fake.identities.some((i) => i.person_id === UUID && i.identity_type === 'email'),
    false,
  )
  assert.equal(
    fake.identities.some((i) => i.person_id === UUID && i.identity_type === 'phone'),
    true,
    'phone untouched',
  )
})

test('setClientIdentity is not-found for a missing person', async () => {
  const fake = seededFake()
  await assert.rejects(
    setClientIdentity('00000000-0000-4000-8000-0000000000ff', 'email', 'x@example.com', fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'not-found',
  )
})
