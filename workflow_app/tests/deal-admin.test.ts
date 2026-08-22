import { test } from 'node:test'
import assert from 'node:assert/strict'

import { PortalWriteError } from '../../lib/portal-write-error'
import {
  STRUCTURAL_PARTICIPANT_ROLES,
  isStructuralParticipantRole,
  normalizeDealCreateInput,
  normalizeStructuralParticipantInput,
} from '../../lib/deal-admin'
import {
  createDeal,
  endStructuralParticipant,
  setStructuralParticipant,
} from '../../db/deal-admin-writes'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// OPS-05 — Deal / Participant Administration: targeted unit suite for the
// write seam (createDeal / setStructuralParticipant / endStructuralParticipant)
// and its pure contract (lib/deal-admin.ts). The pure contract is tested
// directly; the SQL seam is exercised through an injected in-memory fake
// TxRunner that models the property/person/app_user/deal/deal_participant
// tables. No database is touched (the imported db/tx.ts is lazy and never
// queried). Real-Postgres coverage of these seams lives in the persistence
// suite (deal-participant invariants).
// ---------------------------------------------------------------------------

type Row = Record<string, any>

const DEAL_UUID = '00000000-0000-4000-8000-000000000001'
const PROPERTY_UUID = '00000000-0000-4000-8000-000000000010'
const PROPERTY_ARCHIVED_UUID = '00000000-0000-4000-8000-000000000011'
const PERSON_UUID = '00000000-0000-4000-8000-000000000020'
const PERSON_ARCHIVED_UUID = '00000000-0000-4000-8000-000000000021'
const PERSON_SECOND_UUID = '00000000-0000-4000-8000-000000000022'
const USER_UUID = '00000000-0000-4000-8000-000000000030'
const USER_SECOND_UUID = '00000000-0000-4000-8000-000000000031'
const USER_INACTIVE_UUID = '00000000-0000-4000-8000-000000000032'
const PARTICIPANT_CLIENT_UUID = '00000000-0000-4000-8000-000000000040'
const PARTICIPANT_OWNER_UUID = '00000000-0000-4000-8000-000000000041'
const PARTICIPANT_SELLER_UUID = '00000000-0000-4000-8000-000000000042'
const PARTICIPANT_OTHER_UUID = '00000000-0000-4000-8000-000000000043'
const PARTICIPANT_MISSING_UUID = '00000000-0000-4000-8000-000000000099'

class FakeDealAdminDb {
  properties: Row[] = []
  people: Row[] = []
  users: Row[] = []
  deals: Row[] = []
  participants: Row[] = []
  seq = 0
  failNextParticipantInsertWithUnique = false

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

    // property probe: select id, archived_at from property where id = $1
    if (t.startsWith('select id, archived_at from property where id = ')) {
      const row = this.properties.find((r) => r.id === p[0])
      return Promise.resolve(
        row ? [{ id: row.id, archived_at: row.archived_at }] : [],
      )
    }
    // person probe: select id, archived_at from person where id = $1
    if (t.startsWith('select id, archived_at from person where id = ')) {
      const row = this.people.find((r) => r.id === p[0])
      return Promise.resolve(
        row ? [{ id: row.id, archived_at: row.archived_at }] : [],
      )
    }
    // app_user probe: select id, active from app_user where id = $1
    if (t.startsWith('select id, active from app_user where id = ')) {
      const row = this.users.find((r) => r.id === p[0])
      return Promise.resolve(row ? [{ id: row.id, active: row.active }] : [])
    }
    // deal exists: select id from deal where id = $1
    if (t.startsWith('select id from deal where id = ')) {
      const row = this.deals.find((r) => r.id === p[0])
      return Promise.resolve(row ? [{ id: row.id }] : [])
    }
    // insert deal: values (id, property_id, client_person_id, owner_user_id, notes)
    if (t.startsWith('insert into deal (')) {
      this.deals.push({
        id: p[0],
        property_id: p[1],
        client_person_id: p[2],
        owner_user_id: p[3],
        notes: p[4],
      })
      return Promise.resolve([])
    }
    // setStructuralParticipant insert (has returning id)
    if (
      t.startsWith(
        'insert into deal_participant (deal_id, person_id, user_id, role, active)',
      ) &&
      t.includes('returning id')
    ) {
      if (this.failNextParticipantInsertWithUnique) {
        this.failNextParticipantInsertWithUnique = false
        return Promise.reject({ code: '23505' })
      }
      const row = {
        id: `p-${++this.seq}`,
        deal_id: p[0],
        person_id: p[1] ?? null,
        user_id: p[2] ?? null,
        role: p[3],
        role_label: null,
        active: true,
        ended_at: null,
      }
      this.participants.push(row)
      return Promise.resolve([{ id: row.id }])
    }
    // createDeal participant inserts (no returning id). The role/active values
    // are SQL literals in the seam ('client'/'owner', true), so only the
    // deal/subject ids arrive as parameters.
    if (
      t.startsWith(
        'insert into deal_participant (deal_id, person_id, role, active)',
      )
    ) {
      this.participants.push({
        id: `p-${++this.seq}`,
        deal_id: p[0],
        person_id: p[1] ?? null,
        user_id: null,
        role: 'client',
        role_label: null,
        active: true,
        ended_at: null,
      })
      return Promise.resolve([])
    }
    if (
      t.startsWith(
        'insert into deal_participant (deal_id, user_id, role, active)',
      )
    ) {
      this.participants.push({
        id: `p-${++this.seq}`,
        deal_id: p[0],
        person_id: null,
        user_id: p[1] ?? null,
        role: 'owner',
        role_label: null,
        active: true,
        ended_at: null,
      })
      return Promise.resolve([])
    }
    // end current same-role participant (setStructuralParticipant)
    if (
      t.startsWith('update deal_participant set active = false') &&
      t.includes('where deal_id = $1 and role = $2 and active = true')
    ) {
      for (const row of this.participants) {
        if (
          row.deal_id === p[0] &&
          row.role === p[1] &&
          row.active === true
        ) {
          row.active = false
          row.ended_at = 'now'
        }
      }
      return Promise.resolve([])
    }
    // end participant by id (endStructuralParticipant)
    if (
      t.startsWith('update deal_participant set active = false') &&
      t.includes('where id = $1 and active = true')
    ) {
      const row = this.participants.find(
        (r) => r.id === p[0] && r.active === true,
      )
      if (!row) return Promise.resolve([])
      row.active = false
      row.ended_at = 'now'
      return Promise.resolve([{ id: row.id }])
    }
    // client mirror: update deal set client_person_id = $1 ... where id = $2
    if (t.startsWith('update deal set client_person_id = ')) {
      const deal = this.deals.find((r) => r.id === p[1])
      if (deal) deal.client_person_id = p[0]
      return Promise.resolve([])
    }
    // owner clear: update deal set owner_user_id = null ... where id = $1 and owner_user_id = $2
    if (t.startsWith('update deal set owner_user_id = null')) {
      const deal = this.deals.find(
        (r) => r.id === p[0] && r.owner_user_id === p[1],
      )
      if (deal) deal.owner_user_id = null
      return Promise.resolve([])
    }
    // owner mirror: update deal set owner_user_id = $1 ... where id = $2
    if (t.startsWith('update deal set owner_user_id = ')) {
      const deal = this.deals.find((r) => r.id === p[1])
      if (deal) deal.owner_user_id = p[0]
      return Promise.resolve([])
    }
    // participant lookup for endStructuralParticipant
    if (
      t.startsWith(
        'select id, deal_id, role, person_id, user_id from deal_participant',
      )
    ) {
      const row = this.participants.find(
        (r) => r.id === p[0] && r.active === true,
      )
      return Promise.resolve(
        row
          ? [
              {
                id: row.id,
                deal_id: row.deal_id,
                role: row.role,
                person_id: row.person_id,
                user_id: row.user_id,
              },
            ]
          : [],
      )
    }

    throw new Error(`FAKE_DEAL_ADMIN_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

function seededFake() {
  const fake = new FakeDealAdminDb()
  fake.properties.push(
    { id: PROPERTY_UUID, archived_at: null },
    { id: PROPERTY_ARCHIVED_UUID, archived_at: '2026-08-01T00:00:00Z' },
  )
  fake.people.push(
    { id: PERSON_UUID, archived_at: null },
    { id: PERSON_ARCHIVED_UUID, archived_at: '2026-08-01T00:00:00Z' },
    { id: PERSON_SECOND_UUID, archived_at: null },
  )
  fake.users.push(
    { id: USER_UUID, active: true },
    { id: USER_SECOND_UUID, active: true },
    { id: USER_INACTIVE_UUID, active: false },
  )
  fake.deals.push({
    id: DEAL_UUID,
    property_id: PROPERTY_UUID,
    client_person_id: PERSON_UUID,
    owner_user_id: USER_UUID,
    notes: null,
  })
  fake.participants.push(
    {
      id: PARTICIPANT_CLIENT_UUID,
      deal_id: DEAL_UUID,
      person_id: PERSON_UUID,
      user_id: null,
      role: 'client',
      role_label: null,
      active: true,
      ended_at: null,
    },
    {
      id: PARTICIPANT_OWNER_UUID,
      deal_id: DEAL_UUID,
      person_id: null,
      user_id: USER_UUID,
      role: 'owner',
      role_label: null,
      active: true,
      ended_at: null,
    },
  )
  return fake
}

function isValidation(err: unknown) {
  return err instanceof PortalWriteError && err.code === 'validation'
}

function isNotFound(err: unknown) {
  return err instanceof PortalWriteError && err.code === 'not-found'
}

function isConflict(err: unknown) {
  return err instanceof PortalWriteError && err.code === 'conflict'
}

// ---------------------------------------------------------------------------
// Pure contract
// ---------------------------------------------------------------------------

test('normalizeDealCreateInput normalizes a full valid input', () => {
  const out = normalizeDealCreateInput({
    propertyId: PROPERTY_UUID,
    clientPersonId: PERSON_UUID,
    ownerUserId: USER_UUID,
    notes: '  Buyer is pre-qualified.  ',
  })
  assert.equal(out.propertyId, PROPERTY_UUID)
  assert.equal(out.clientPersonId, PERSON_UUID)
  assert.equal(out.ownerUserId, USER_UUID)
  assert.equal(out.notes, 'Buyer is pre-qualified.')
})

test('normalizeDealCreateInput clears empty owner and notes to null', () => {
  const out = normalizeDealCreateInput({
    propertyId: PROPERTY_UUID,
    clientPersonId: PERSON_UUID,
    ownerUserId: '',
    notes: '   ',
  })
  assert.equal(out.ownerUserId, null)
  assert.equal(out.notes, null)
})

test('normalizeDealCreateInput rejects non-uuid property and client identifiers', () => {
  assert.throws(
    () =>
      normalizeDealCreateInput({
        propertyId: 'not-a-uuid',
        clientPersonId: PERSON_UUID,
      }),
    isValidation,
  )
  assert.throws(
    () =>
      normalizeDealCreateInput({
        propertyId: PROPERTY_UUID,
        clientPersonId: 'not-a-uuid',
      }),
    isValidation,
  )
})

test('normalizeStructuralParticipantInput accepts each structural role with its fixed subject kind', () => {
  const client = normalizeStructuralParticipantInput({
    dealId: DEAL_UUID,
    role: 'client',
    personId: PERSON_UUID,
  })
  assert.equal(client.kind, 'person')
  assert.equal(client.personId, PERSON_UUID)
  assert.equal(client.userId, null)

  const owner = normalizeStructuralParticipantInput({
    dealId: DEAL_UUID,
    role: 'owner',
    userId: USER_UUID,
  })
  assert.equal(owner.kind, 'user')
  assert.equal(owner.userId, USER_UUID)
  assert.equal(owner.personId, null)

  const seller = normalizeStructuralParticipantInput({
    dealId: DEAL_UUID,
    role: 'seller',
    personId: PERSON_UUID,
  })
  assert.equal(seller.kind, 'person')
  assert.equal(seller.personId, PERSON_UUID)
})

test('normalizeStructuralParticipantInput rejects an unknown role', () => {
  assert.throws(
    () =>
      normalizeStructuralParticipantInput({
        dealId: DEAL_UUID,
        role: 'lender' as never,
        personId: PERSON_UUID,
      }),
    isValidation,
  )
})

test('normalizeStructuralParticipantInput enforces the subject kind per role', () => {
  // client/seller are people — a user is refused.
  assert.throws(
    () =>
      normalizeStructuralParticipantInput({
        dealId: DEAL_UUID,
        role: 'client',
        userId: USER_UUID,
      }),
    isValidation,
  )
  // owner is an app user — a person is refused.
  assert.throws(
    () =>
      normalizeStructuralParticipantInput({
        dealId: DEAL_UUID,
        role: 'owner',
        personId: PERSON_UUID,
      }),
    isValidation,
  )
})

test('normalizeStructuralParticipantInput rejects both or neither subject', () => {
  assert.throws(
    () =>
      normalizeStructuralParticipantInput({
        dealId: DEAL_UUID,
        role: 'seller',
        personId: PERSON_UUID,
        userId: USER_UUID,
      }),
    isValidation,
  )
  assert.throws(
    () =>
      normalizeStructuralParticipantInput({
        dealId: DEAL_UUID,
        role: 'client',
      }),
    isValidation,
  )
})

test('structural role vocabulary matches the migration 034 roles', () => {
  assert.deepEqual(STRUCTURAL_PARTICIPANT_ROLES, ['client', 'owner', 'seller'])
  for (const role of ['client', 'owner', 'seller']) {
    assert.ok(isStructuralParticipantRole(role), role)
  }
  assert.ok(!isStructuralParticipantRole('other'))
})

// ---------------------------------------------------------------------------
// createDeal
// ---------------------------------------------------------------------------

test('createDeal inserts the deal and both canonical participants atomically', async () => {
  const fake = seededFake()
  const { id } = await createDeal(
    {
      propertyId: PROPERTY_UUID,
      clientPersonId: PERSON_UUID,
      ownerUserId: USER_UUID,
      notes: '  New opportunity  ',
    },
    fake.runner,
  )

  const deal = fake.deals.find((d) => d.id === id)
  assert.ok(deal, 'deal row created')
  assert.equal(deal.property_id, PROPERTY_UUID)
  assert.equal(deal.client_person_id, PERSON_UUID)
  assert.equal(deal.owner_user_id, USER_UUID)
  assert.equal(deal.notes, 'New opportunity')

  const client = fake.participants.find(
    (p) => p.deal_id === id && p.role === 'client',
  )
  assert.ok(client && client.active === true)
  assert.equal(client.person_id, PERSON_UUID)

  const owner = fake.participants.find(
    (p) => p.deal_id === id && p.role === 'owner',
  )
  assert.ok(owner && owner.active === true)
  assert.equal(owner.user_id, USER_UUID)
})

test('createDeal without an owner creates only the client participant', async () => {
  const fake = seededFake()
  const { id } = await createDeal(
    {
      propertyId: PROPERTY_UUID,
      clientPersonId: PERSON_UUID,
    },
    fake.runner,
  )

  const deal = fake.deals.find((d) => d.id === id)
  assert.equal(deal.owner_user_id, null)
  const owners = fake.participants.filter(
    (p) => p.deal_id === id && p.role === 'owner',
  )
  assert.equal(owners.length, 0)
})

test('createDeal is not-found for a missing property', async () => {
  const fake = seededFake()
  await assert.rejects(
    createDeal(
      {
        propertyId: '00000000-0000-4000-8000-000000000099',
        clientPersonId: PERSON_UUID,
      },
      fake.runner,
    ),
    isNotFound,
  )
})

test('createDeal refuses an archived property', async () => {
  const fake = seededFake()
  await assert.rejects(
    createDeal(
      {
        propertyId: PROPERTY_ARCHIVED_UUID,
        clientPersonId: PERSON_UUID,
      },
      fake.runner,
    ),
    isConflict,
  )
})

test('createDeal is not-found for a missing client person', async () => {
  const fake = seededFake()
  await assert.rejects(
    createDeal(
      {
        propertyId: PROPERTY_UUID,
        clientPersonId: '00000000-0000-4000-8000-000000000098',
      },
      fake.runner,
    ),
    isNotFound,
  )
})

test('createDeal refuses an archived client person', async () => {
  const fake = seededFake()
  await assert.rejects(
    createDeal(
      {
        propertyId: PROPERTY_UUID,
        clientPersonId: PERSON_ARCHIVED_UUID,
      },
      fake.runner,
    ),
    isConflict,
  )
})

test('createDeal is not-found for a missing owner user', async () => {
  const fake = seededFake()
  await assert.rejects(
    createDeal(
      {
        propertyId: PROPERTY_UUID,
        clientPersonId: PERSON_UUID,
        ownerUserId: '00000000-0000-4000-8000-000000000097',
      },
      fake.runner,
    ),
    isNotFound,
  )
})

test('createDeal refuses an inactive owner user', async () => {
  const fake = seededFake()
  await assert.rejects(
    createDeal(
      {
        propertyId: PROPERTY_UUID,
        clientPersonId: PERSON_UUID,
        ownerUserId: USER_INACTIVE_UUID,
      },
      fake.runner,
    ),
    isConflict,
  )
})

// ---------------------------------------------------------------------------
// setStructuralParticipant
// ---------------------------------------------------------------------------

test('setStructuralParticipant replaces the client and syncs the deal FK mirror', async () => {
  const fake = seededFake()
  const { participantId } = await setStructuralParticipant(
    { dealId: DEAL_UUID, role: 'client', personId: PERSON_SECOND_UUID },
    fake.runner,
  )

  const oldClient = fake.participants.find((p) => p.id === PARTICIPANT_CLIENT_UUID)
  assert.equal(oldClient.active, false, 'old client row is ended')
  assert.ok(oldClient.ended_at !== null)

  const newClient = fake.participants.find((p) => p.id === participantId)
  assert.ok(newClient, 'new client row exists')
  assert.equal(newClient.active, true)
  assert.equal(newClient.role, 'client')
  assert.equal(newClient.person_id, PERSON_SECOND_UUID)

  const deal = fake.deals.find((d) => d.id === DEAL_UUID)
  assert.equal(deal.client_person_id, PERSON_SECOND_UUID, 'legacy mirror synced')

  const activeClients = fake.participants.filter(
    (p) => p.deal_id === DEAL_UUID && p.role === 'client' && p.active === true,
  )
  assert.equal(activeClients.length, 1, 'one active client invariant holds')
})

test('setStructuralParticipant replaces the owner and syncs the deal FK mirror', async () => {
  const fake = seededFake()
  const { participantId } = await setStructuralParticipant(
    { dealId: DEAL_UUID, role: 'owner', userId: USER_SECOND_UUID },
    fake.runner,
  )

  const newOwner = fake.participants.find((p) => p.id === participantId)
  assert.equal(newOwner.user_id, USER_SECOND_UUID)
  assert.equal(newOwner.active, true)

  const deal = fake.deals.find((d) => d.id === DEAL_UUID)
  assert.equal(deal.owner_user_id, USER_SECOND_UUID)
})

test('setStructuralParticipant sets a seller without touching property facts', async () => {
  const fake = seededFake()
  const { participantId } = await setStructuralParticipant(
    { dealId: DEAL_UUID, role: 'seller', personId: PERSON_SECOND_UUID },
    fake.runner,
  )

  const seller = fake.participants.find((p) => p.id === participantId)
  assert.equal(seller.role, 'seller')
  assert.equal(seller.person_id, PERSON_SECOND_UUID)
  assert.equal(seller.active, true)

  // The seller mirror lives on property (property-domain fact) and is never
  // rewritten by deal-participant maintenance.
  const property = fake.properties.find((p) => p.id === PROPERTY_UUID)
  assert.ok(!('seller_person_id' in property))
})

test('setStructuralParticipant is not-found for a missing deal', async () => {
  const fake = seededFake()
  await assert.rejects(
    setStructuralParticipant(
      {
        dealId: '00000000-0000-4000-8000-000000000096',
        role: 'client',
        personId: PERSON_UUID,
      },
      fake.runner,
    ),
    isNotFound,
  )
})

test('setStructuralParticipant refuses an archived person', async () => {
  const fake = seededFake()
  await assert.rejects(
    setStructuralParticipant(
      { dealId: DEAL_UUID, role: 'seller', personId: PERSON_ARCHIVED_UUID },
      fake.runner,
    ),
    isConflict,
  )
})

test('setStructuralParticipant refuses an inactive owner user', async () => {
  const fake = seededFake()
  await assert.rejects(
    setStructuralParticipant(
      { dealId: DEAL_UUID, role: 'owner', userId: USER_INACTIVE_UUID },
      fake.runner,
    ),
    isConflict,
  )
})

test('setStructuralParticipant maps a unique-violation race to a conflict', async () => {
  const fake = seededFake()
  fake.failNextParticipantInsertWithUnique = true
  await assert.rejects(
    setStructuralParticipant(
      { dealId: DEAL_UUID, role: 'seller', personId: PERSON_UUID },
      fake.runner,
    ),
    isConflict,
  )
})

// ---------------------------------------------------------------------------
// endStructuralParticipant
// ---------------------------------------------------------------------------

test('endStructuralParticipant ends an owner and clears the guarded deal FK mirror', async () => {
  const fake = seededFake()
  await endStructuralParticipant(PARTICIPANT_OWNER_UUID, fake.runner)

  const owner = fake.participants.find((p) => p.id === PARTICIPANT_OWNER_UUID)
  assert.equal(owner.active, false)
  assert.ok(owner.ended_at !== null)

  const deal = fake.deals.find((d) => d.id === DEAL_UUID)
  assert.equal(deal.owner_user_id, null, 'owner mirror cleared')
})

test('endStructuralParticipant ends a seller without touching property facts', async () => {
  const fake = seededFake()
  fake.participants.push({
    id: PARTICIPANT_SELLER_UUID,
    deal_id: DEAL_UUID,
    person_id: PERSON_SECOND_UUID,
    user_id: null,
    role: 'seller',
    role_label: null,
    active: true,
    ended_at: null,
  })
  await endStructuralParticipant(PARTICIPANT_SELLER_UUID, fake.runner)

  const seller = fake.participants.find((p) => p.id === PARTICIPANT_SELLER_UUID)
  assert.equal(seller.active, false)

  const property = fake.properties.find((p) => p.id === PROPERTY_UUID)
  assert.ok(!('seller_person_id' in property))
})

test('endStructuralParticipant refuses to end a client', async () => {
  const fake = seededFake()
  await assert.rejects(
    endStructuralParticipant(PARTICIPANT_CLIENT_UUID, fake.runner),
    isConflict,
  )
  const client = fake.participants.find((p) => p.id === PARTICIPANT_CLIENT_UUID)
  assert.equal(client.active, true, 'client stays active')
})

test('endStructuralParticipant is not-found for a missing participant', async () => {
  const fake = seededFake()
  await assert.rejects(
    endStructuralParticipant(PARTICIPANT_MISSING_UUID, fake.runner),
    isNotFound,
  )
})

test('endStructuralParticipant refuses a long-tail (role=other) participant', async () => {
  const fake = seededFake()
  fake.participants.push({
    id: PARTICIPANT_OTHER_UUID,
    deal_id: DEAL_UUID,
    person_id: PERSON_UUID,
    user_id: null,
    role: 'other',
    role_label: 'Attorney',
    active: true,
    ended_at: null,
  })
  await assert.rejects(
    endStructuralParticipant(PARTICIPANT_OTHER_UUID, fake.runner),
    isConflict,
  )
})

test('endStructuralParticipant rejects an invalid participant id', async () => {
  const fake = seededFake()
  await assert.rejects(
    endStructuralParticipant('not-a-uuid', fake.runner),
    isValidation,
  )
})
