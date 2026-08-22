import { test } from 'node:test'
import assert from 'node:assert/strict'

import { PortalWriteError } from '../../lib/portal-write-error'
import {
  EDITABLE_PROPERTY_STATUSES,
  PROPERTY_STATUSES,
  normalizePropertyCreateInput,
  normalizePropertySlug,
} from '../../lib/property-admin'
import {
  archiveProperty,
  createProperty,
  restoreProperty,
} from '../../db/property-admin-writes'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// OPS-03 — Property Administration: targeted unit suite for the property CRUD
// lifecycle seam (create / archive / restore). The pure contract
// (lib/property-admin.ts) is tested directly; the SQL seam
// (db/property-admin-writes.ts) is exercised through an injected in-memory
// fake TxRunner that models the property table. No database is touched (the
// imported db/tx.ts is lazy and never queried). Real-Postgres coverage of
// these seams lives in the persistence suite.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

const UUID = '00000000-0000-4000-8000-000000000001'

class FakePropertyAdmin {
  properties: Row[] = []
  failNextInsertWithUnique = false

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

    // propertyExists
    if (t.startsWith('select id from property where id = ')) {
      const row = this.properties.find((r) => r.id === p[0])
      return Promise.resolve(row ? [{ id: row.id }] : [])
    }
    // slug clash probe (create)
    if (t.startsWith('select id from property where slug = ')) {
      const row = this.properties.find((r) => r.slug === p[0])
      return Promise.resolve(row ? [{ id: row.id }] : [])
    }
    // insert new property (city/state coalesced to the schema defaults)
    if (t.startsWith('insert into property (')) {
      if (this.failNextInsertWithUnique) {
        this.failNextInsertWithUnique = false
        return Promise.reject({ code: '23505' })
      }
      this.properties.push({
        id: p[0],
        name: p[1],
        slug: p[2],
        status: p[3],
        featured: p[4],
        property_type: p[5],
        list_price: p[6],
        location: p[7],
        city: p[8] ?? 'Culebra',
        state_or_province: p[9] ?? 'PR',
        neighborhood: p[10],
        bedrooms: p[11],
        bathrooms: p[12],
        square_feet: p[13],
        archived_at: null,
      })
      return Promise.resolve([])
    }
    // archive (soft delete)
    if (
      t.startsWith('update property set archived_at = now()') &&
      t.includes('and archived_at is null')
    ) {
      const row = this.properties.find(
        (r) => r.id === p[0] && r.archived_at === null,
      )
      if (!row) return Promise.resolve([])
      row.archived_at = new Date().toISOString()
      return Promise.resolve([{ id: row.id }])
    }
    // restore (clear the soft-delete flag)
    if (t.startsWith('update property set archived_at = null')) {
      const row = this.properties.find(
        (r) => r.id === p[0] && r.archived_at !== null,
      )
      if (!row) return Promise.resolve([])
      row.archived_at = null
      return Promise.resolve([{ id: row.id }])
    }

    throw new Error(`FAKE_PROPERTY_ADMIN_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

function seededFake() {
  const fake = new FakePropertyAdmin()
  fake.properties.push({
    id: UUID,
    name: 'Existing Villa',
    slug: 'existing-villa',
    status: 'active',
    featured: false,
    property_type: 'Villa',
    list_price: '1250000',
    location: 'Flamenco',
    city: 'Culebra',
    state_or_province: 'PR',
    neighborhood: null,
    bedrooms: '3',
    bathrooms: '2',
    square_feet: 1800,
    archived_at: null,
  })
  return fake
}

// ---------------------------------------------------------------------------
// Pure contract — lib/property-admin.ts
// ---------------------------------------------------------------------------

test('normalizePropertyCreateInput normalizes a full valid input', () => {
  const input = normalizePropertyCreateInput({
    name: '  Casa del Sol  ',
    slug: 'casa-del-sol',
  })
  assert.equal(input.name, 'Casa del Sol')
  assert.equal(input.slug, 'casa-del-sol')
  assert.equal(input.status, 'prospect')
  assert.equal(input.featured, false)
})

test('normalizePropertyCreateInput trims and maps every provided field', () => {
  const input = normalizePropertyCreateInput({
    name: 'Casa del Sol',
    slug: 'casa-del-sol',
    status: 'coming_soon',
    featured: true,
    propertyType: ' Villa ',
    listPrice: 1250000,
    location: ' Flamenco ',
    city: 'Culebra',
    stateOrProvince: 'PR',
    neighborhood: ' Flamenco Beach ',
    bedrooms: 3,
    bathrooms: 2.5,
    squareFeet: 1800,
  })
  assert.deepEqual(input, {
    name: 'Casa del Sol',
    slug: 'casa-del-sol',
    status: 'coming_soon',
    featured: true,
    propertyType: 'Villa',
    listPrice: 1250000,
    location: 'Flamenco',
    city: 'Culebra',
    stateOrProvince: 'PR',
    neighborhood: 'Flamenco Beach',
    bedrooms: 3,
    bathrooms: 2.5,
    squareFeet: 1800,
  })
})

test('normalizePropertyCreateInput defaults status to prospect and featured to false', () => {
  const input = normalizePropertyCreateInput({ name: 'No Status' })
  assert.equal(input.status, 'prospect')
  assert.equal(input.featured, false)
  assert.equal(input.city, null, 'db seam coalesces city to the schema default')
  assert.equal(input.stateOrProvince, null)
})

test('normalizePropertyCreateInput rejects a missing name', () => {
  assert.throws(
    () => normalizePropertyCreateInput({ name: '   ' }),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'validation' &&
      /name/i.test(error.message),
  )
})

test('normalizePropertyCreateInput rejects an invalid slug', () => {
  assert.throws(
    () => normalizePropertyCreateInput({ name: 'X', slug: 'Casa Del Sol' }),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'validation' &&
      /slug/i.test(error.message),
  )
})

test('normalizePropertySlug accepts lowercase hyphenated slugs and clears empty', () => {
  assert.equal(normalizePropertySlug('casa-del-sol'), 'casa-del-sol')
  assert.equal(normalizePropertySlug('  '), null)
  assert.equal(normalizePropertySlug(null), null)
})

test('normalizePropertyCreateInput rejects a transaction status', () => {
  for (const status of ['under_contract', 'sold', 'archived']) {
    assert.throws(
      () => normalizePropertyCreateInput({ name: 'X', status } as never),
      (error: unknown) =>
        error instanceof PortalWriteError &&
        error.code === 'validation' &&
        /status/i.test(error.message),
      `status ${status} must be rejected from listing administration`,
    )
  }
})

test('normalizePropertyCreateInput rejects negative numerics', () => {
  assert.throws(
    () => normalizePropertyCreateInput({ name: 'X', listPrice: -1 }),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'validation',
  )
  assert.throws(
    () => normalizePropertyCreateInput({ name: 'X', bedrooms: -2 }),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'validation',
  )
})

test('property status vocabularies match the property schema', () => {
  assert.deepEqual([...EDITABLE_PROPERTY_STATUSES], [
    'prospect',
    'coming_soon',
    'active',
    'off_market',
  ])
  assert.deepEqual([...PROPERTY_STATUSES], [
    'prospect',
    'coming_soon',
    'active',
    'off_market',
    'under_contract',
    'sold',
    'archived',
  ])
})

// ---------------------------------------------------------------------------
// DB seam — db/property-admin-writes.ts through the fake TxRunner
// ---------------------------------------------------------------------------

test('createProperty inserts a canonical property row with defaults', async () => {
  const fake = seededFake()

  const result = await createProperty(
    {
      name: 'Casa del Sol',
      status: 'coming_soon',
      listPrice: 950000,
      location: 'Flamenco',
    },
    fake.runner,
  )

  assert.ok(result.id)
  assert.equal(fake.properties.length, 2)
  const created = fake.properties.find((p) => p.id === result.id)!
  assert.equal(created.name, 'Casa del Sol')
  assert.equal(created.status, 'coming_soon')
  assert.equal(created.featured, false)
  assert.equal(created.city, 'Culebra', 'schema default applied')
  assert.equal(created.state_or_province, 'PR', 'schema default applied')
  assert.equal(created.archived_at, null)
})

test('createProperty inserts with every provided field', async () => {
  const fake = seededFake()

  const result = await createProperty(
    {
      name: 'Casa del Sol',
      slug: 'casa-del-sol',
      status: 'active',
      featured: true,
      propertyType: 'Villa',
      listPrice: 1250000,
      location: 'Flamenco',
      city: 'Culebra',
      stateOrProvince: 'PR',
      neighborhood: 'Flamenco Beach',
      bedrooms: 3,
      bathrooms: 2,
      squareFeet: 1800,
    },
    fake.runner,
  )

  const created = fake.properties.find((p) => p.id === result.id)!
  assert.equal(created.slug, 'casa-del-sol')
  assert.equal(created.status, 'active')
  assert.equal(created.featured, true)
  assert.equal(created.property_type, 'Villa')
  assert.equal(created.list_price, 1250000)
  assert.equal(created.bedrooms, 3)
  assert.equal(created.square_feet, 1800)
})

test('createProperty refuses a slug already in use', async () => {
  const fake = seededFake()

  await assert.rejects(
    createProperty({ name: 'Clone', slug: 'existing-villa' }, fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'conflict' &&
      /slug/i.test(error.message),
  )
  assert.equal(fake.properties.length, 1, 'no row inserted on conflict')
})

test('createProperty maps a unique-race to a slug conflict', async () => {
  const fake = seededFake()
  fake.failNextInsertWithUnique = true

  await assert.rejects(
    createProperty({ name: 'Clone', slug: 'casa-del-sol' }, fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError &&
      error.code === 'conflict' &&
      /slug/i.test(error.message),
  )
})

test('createProperty rejects a missing name before any query', async () => {
  const fake = seededFake()
  await assert.rejects(
    createProperty({ name: '   ' }, fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'validation',
  )
  assert.equal(fake.properties.length, 1)
})

test('archiveProperty soft-deletes and is a conflict on the second call', async () => {
  const fake = seededFake()

  await archiveProperty(UUID, fake.runner)
  assert.ok(fake.properties.find((p) => p.id === UUID)!.archived_at)

  await assert.rejects(
    archiveProperty(UUID, fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'conflict',
  )
})

test('archiveProperty is not-found for a missing property', async () => {
  const fake = seededFake()
  await assert.rejects(
    archiveProperty('00000000-0000-4000-8000-0000000000ff', fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'not-found',
  )
})

test('archiveProperty rejects an invalid identifier', async () => {
  const fake = seededFake()
  await assert.rejects(
    archiveProperty('not-a-uuid', fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'validation',
  )
})

test('restoreProperty clears the soft-delete flag', async () => {
  const fake = seededFake()
  const row = fake.properties.find((p) => p.id === UUID)!
  row.archived_at = new Date().toISOString()

  await restoreProperty(UUID, fake.runner)
  assert.equal(row.archived_at, null)
})

test('restoreProperty is a conflict when the property is not archived', async () => {
  const fake = seededFake()
  await assert.rejects(
    restoreProperty(UUID, fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'conflict',
  )
})

test('restoreProperty is not-found for a missing property', async () => {
  const fake = seededFake()
  await assert.rejects(
    restoreProperty('00000000-0000-4000-8000-0000000000ff', fake.runner),
    (error: unknown) =>
      error instanceof PortalWriteError && error.code === 'not-found',
  )
})

test('archive then restore round-trips and archive is allowed again', async () => {
  const fake = seededFake()

  await archiveProperty(UUID, fake.runner)
  const row = fake.properties.find((p) => p.id === UUID)!
  assert.ok(row.archived_at)

  await restoreProperty(UUID, fake.runner)
  assert.equal(row.archived_at, null)

  await archiveProperty(UUID, fake.runner)
  assert.ok(row.archived_at, 're-archive after restore succeeds')
})
