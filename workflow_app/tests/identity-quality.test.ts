import { test } from 'node:test'
import assert from 'node:assert/strict'

import { getIdentityQuality } from '../../db/identity-quality'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// CRM-18 — identity-quality projection (zero Neon).
//
// The projection runs three queries through an injected QueryExecutor; the
// fake below dispatches on SQL shape and returns canned rows. The pure
// derivation logic (coverage counts, malformed-identity flags via the strict
// email / E.164 rules, weak-coverage filter, type distribution) is what these
// tests verify. The SQL text itself is exercised against the DEV database in
// the projection's live run; here we additionally assert the type-count query
// excludes archived people (the CRM-18 determinism fix).
// ---------------------------------------------------------------------------

type Row = Record<string, unknown>

function norm(s: string) {
  return s.replace(/\s+/g, ' ').trim().toLowerCase()
}

class FakeDb {
  personRows: Row[] = []
  typeRows: Row[] = []
  identityRows: Row[] = []
  queries: string[] = []

  tx: QueryExecutor = (strings, ...params) => {
    const text = norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < params.length ? `$${i + 1}` : ''),
        '',
      ),
    )
    this.queries.push(text)

    if (text.includes('from person p') && text.includes('left join person_identity')) {
      return Promise.resolve(this.personRows)
    }
    if (text.includes('group by pi.identity_type')) {
      return Promise.resolve(this.typeRows)
    }
    if (text.includes('pi.identity_value')) {
      return Promise.resolve(this.identityRows)
    }
    return Promise.resolve([])
  }
}

const personRow = (
  overrides: Partial<Row> = {},
): Row => ({
  id: 'p-1',
  display_name: 'Ada Lovelace',
  role: 'buyer',
  status: 'active',
  identity_count: 2,
  email_count: 1,
  primary_email_count: 1,
  phone_count: 1,
  primary_phone_count: 1,
  active_deal_count: 0,
  open_task_count: 0,
  ...overrides,
})

test('coverage metrics derive from the person summary rows', async () => {
  const f = new FakeDb()
  f.personRows = [
    personRow({ id: 'p-1', display_name: 'Full Coverage', email_count: 1, primary_email_count: 1, phone_count: 1, primary_phone_count: 1, identity_count: 2 }),
    personRow({ id: 'p-2', display_name: 'No Identity', identity_count: 0, email_count: 0, phone_count: 0 }),
    personRow({ id: 'p-3', display_name: 'No Email', identity_count: 1, email_count: 0, phone_count: 1, primary_phone_count: 1 }),
    personRow({ id: 'p-4', display_name: 'No Phone', identity_count: 1, email_count: 1, primary_email_count: 1, phone_count: 0 }),
    personRow({ id: 'p-5', display_name: 'No Primary Email', identity_count: 1, email_count: 1, primary_email_count: 0 }),
    personRow({ id: 'p-6', display_name: 'No Primary Phone', identity_count: 1, email_count: 1, primary_email_count: 1, phone_count: 1, primary_phone_count: 0 }),
  ]

  const snap = await getIdentityQuality(f.tx)

  assert.equal(snap.totalPeople, 6)
  assert.equal(snap.peopleWithNoIdentity, 1)
  assert.equal(snap.peopleWithoutEmail, 2)
  assert.equal(snap.peopleWithoutPrimaryEmail, 1)
  assert.equal(snap.peopleWithoutPhone, 2)
  assert.equal(snap.peopleWithoutPrimaryPhone, 1)
})

test('malformed identities are flagged with the strict email / E.164 rules', async () => {
  const f = new FakeDb()
  f.personRows = [personRow()]
  f.identityRows = [
    { person_id: 'p-1', person_name: 'Ada Lovelace', identity_type: 'email', identity_value: 'ada@example.com' },
    { person_id: 'p-1', person_name: 'Ada Lovelace', identity_type: 'email', identity_value: 'not-an-email' },
    { person_id: 'p-1', person_name: 'Ada Lovelace', identity_type: 'phone', identity_value: '+15551234567' },
    { person_id: 'p-1', person_name: 'Ada Lovelace', identity_type: 'phone', identity_value: '555-1234' },
    { person_id: 'p-1', person_name: 'Ada Lovelace', identity_type: 'external', identity_value: 'hubspot:abc' },
  ]

  const snap = await getIdentityQuality(f.tx)

  assert.equal(snap.malformedIdentities.length, 2)
  const byType = new Map(
    snap.malformedIdentities.map((m) => [m.identityType, m]),
  )
  assert.equal(byType.get('email')?.value, 'not-an-email')
  assert.equal(byType.get('email')?.issue, 'invalid_email')
  assert.equal(byType.get('phone')?.value, '555-1234')
  assert.equal(byType.get('phone')?.issue, 'invalid_phone')
})

test('weak coverage includes only active-work people with no email AND no phone', async () => {
  const f = new FakeDb()
  f.personRows = [
    personRow({ id: 'p-1', display_name: 'Weak Contact', active_deal_count: 1, open_task_count: 0, identity_count: 0, email_count: 0, phone_count: 0 }),
    personRow({ id: 'p-2', display_name: 'Open Task Only', active_deal_count: 0, open_task_count: 2, identity_count: 0, email_count: 0, phone_count: 0 }),
    personRow({ id: 'p-3', display_name: 'Has Email', active_deal_count: 1, email_count: 1, primary_email_count: 1, phone_count: 0 }),
    personRow({ id: 'p-4', display_name: 'No Active Work', active_deal_count: 0, open_task_count: 0, email_count: 0, phone_count: 0 }),
  ]

  const snap = await getIdentityQuality(f.tx)

  assert.deepEqual(
    snap.weakCoverage.map((p) => p.displayName),
    ['Weak Contact', 'Open Task Only'],
  )
  assert.equal(snap.weakCoverage[0].activeDealCount, 1)
  assert.equal(snap.weakCoverage[0].openTaskCount, 0)
  assert.equal(snap.weakCoverage[1].openTaskCount, 2)
})

test('identity type distribution maps rows and exact duplicates are structurally impossible', async () => {
  const f = new FakeDb()
  f.personRows = [personRow()]
  f.typeRows = [
    { identity_type: 'email', count: 4 },
    { identity_type: 'phone', count: 4 },
  ]

  const snap = await getIdentityQuality(f.tx)

  assert.deepEqual(snap.identityCountByType, [
    { identityType: 'email', count: 4 },
    { identityType: 'phone', count: 4 },
  ])
  assert.equal(snap.exactDuplicateCheck.possible, false)
  assert.match(snap.exactDuplicateCheck.note, /UNIQUE/)
})

test('type-count query excludes archived people (determinism fix)', async () => {
  const f = new FakeDb()
  f.personRows = [personRow()]
  f.typeRows = [{ identity_type: 'email', count: 1 }]

  await getIdentityQuality(f.tx)

  const typeQuery = f.queries.find((q) => q.includes('group by pi.identity_type'))
  assert.ok(typeQuery, 'type-count query was issued')
  assert.ok(
    typeQuery!.includes('join person p'),
    'type-count query joins person to scope by archive status',
  )
  assert.ok(
    typeQuery!.includes('where p.archived_at is null'),
    'type-count query excludes archived people',
  )
})
