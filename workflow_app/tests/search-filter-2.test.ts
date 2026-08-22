import { test } from 'node:test'
import assert from 'node:assert/strict'

// PX-24 Buyers Search / Filter 2.0 — canonical client-side inventory pipeline
// (zero Neon, zero React). The server (db/properties.ts getFilteredProperties,
// PX-24B) is the AUTHORITATIVE filter for what renders; this suite proves the
// canonical client-side mirror that the /buyers showroom consumes:
//
//   lib/search-contract.ts:
//     sortProperties          — the canonical order (featured / price-high /
//                               price-low / name) extracted from the showroom
//     applySearchFilters      — match on the SAME contract as the saved-search
//                               matcher, then order canonically, in one step
//     searchParamsToFilters   — the inverse of searchFiltersToQuery, so the
//                               URL round-trip has ONE parse rule (PX-24C)
//
// Key invariants proven here:
//   - the pipeline is idempotent over an already server-filtered list (the
//     showroom double-filter converges — never contradicts the server);
//   - `sort` never changes which properties match (same ids, different order);
//   - the URL round trip (filters -> query -> filters) is lossless for
//     canonical inputs, which the showroom's push-guard depends on.

import type { PropertySummary } from '../../db/properties'
import {
  DEFAULT_SEARCH_FILTERS,
  applySearchFilters,
  searchFiltersToQuery,
  searchParamsToFilters,
  sortProperties,
} from '../../lib/search-contract'
import type { SearchFilters } from '../../lib/search-contract'

const property = (
  overrides: Partial<PropertySummary> = {},
): PropertySummary => ({
  id: 'p-1',
  name: 'Casa Test',
  slug: 'casa-test',
  status: 'Active',
  propertyType: 'house',
  listPrice: 1_500_000,
  featured: false,
  location: 'Flamenco',
  city: 'Culebra',
  neighborhood: null,
  bedrooms: 3,
  bathrooms: 2,
  squareFeet: 2000,
  lotSize: null,
  lotSizeUnits: null,
  views: ['Ocean'],
  waterAccess: false,
  beachAccess: false,
  heroUrl: null,
  heroAlt: 'Casa Test',
  ...overrides,
})

const filters = (
  overrides: Partial<SearchFilters> = {},
): SearchFilters => ({
  ...DEFAULT_SEARCH_FILTERS,
  ...overrides,
})

// A small mixed inventory covering homes, land, prices, beds, and views.
const inventory = (): PropertySummary[] => [
  property({
    id: 'p-home-1',
    name: 'Casa Mar',
    propertyType: 'house',
    listPrice: 3_000_000,
    featured: true,
    bedrooms: 4,
    views: ['Ocean', 'Sunset'],
  }),
  property({
    id: 'p-home-2',
    name: 'Casa Loma',
    propertyType: 'villa',
    listPrice: 1_200_000,
    featured: false,
    bedrooms: 2,
    views: ['Bay'],
  }),
  property({
    id: 'p-home-3',
    name: 'Casa Playa',
    propertyType: 'house',
    listPrice: 2_000_000,
    featured: false,
    bedrooms: 3,
    views: ['Beach'],
  }),
  property({
    id: 'p-home-4',
    name: 'Villa Sin Precio',
    propertyType: 'house',
    listPrice: null,
    featured: false,
    bedrooms: 5,
    views: [],
  }),
  property({
    id: 'p-land-1',
    name: 'Solar Flamenco',
    propertyType: 'land',
    listPrice: 500_000,
    featured: false,
    bedrooms: null,
    views: ['Ocean'],
  }),
]

const ids = (items: PropertySummary[]): string[] =>
  items.map((item) => item.id)

/* ============================================================
   sortProperties — canonical ordering
   ============================================================ */

test('sortProperties: featured puts Featured first, then price desc with nulls last', () => {
  const sorted = sortProperties(inventory(), 'featured')
  assert.deepEqual(ids(sorted), [
    'p-home-1', // featured
    'p-home-3', // 2M
    'p-home-2', // 1.2M
    'p-land-1', // 500k
    'p-home-4', // null price last
  ])
})

test('sortProperties: price-high orders by price desc with nulls last', () => {
  const sorted = sortProperties(inventory(), 'price-high')
  assert.deepEqual(ids(sorted), [
    'p-home-1',
    'p-home-3',
    'p-home-2',
    'p-land-1',
    'p-home-4',
  ])
})

test('sortProperties: price-low orders by price asc with nulls last', () => {
  const sorted = sortProperties(inventory(), 'price-low')
  assert.deepEqual(ids(sorted), [
    'p-land-1',
    'p-home-2',
    'p-home-3',
    'p-home-1',
    'p-home-4',
  ])
})

test('sortProperties: name orders alphabetically regardless of price', () => {
  const sorted = sortProperties(inventory(), 'name')
  assert.deepEqual(ids(sorted), [
    'p-home-2', // Casa Loma
    'p-home-1', // Casa Mar
    'p-home-3', // Casa Playa
    'p-land-1', // Solar Flamenco
    'p-home-4', // Villa Sin Precio
  ])
})

test('sortProperties: never mutates the input array', () => {
  const input = inventory()
  const snapshot = ids(input)
  sortProperties(input, 'price-low')
  sortProperties(input, 'name')
  assert.deepEqual(ids(input), snapshot)
})

test('sortProperties: sort is an ordering, not a filter — every property survives', () => {
  for (const sort of ['featured', 'price-high', 'price-low', 'name'] as const) {
    const sorted = sortProperties(inventory(), sort)
    assert.equal(sorted.length, inventory().length, `sort=${sort} must not drop`)
    assert.deepEqual(
      [...ids(sorted)].sort(),
      [...ids(inventory())].sort(),
      `sort=${sort} must not change the id set`,
    )
  }
})

/* ============================================================
   applySearchFilters — canonical filter + order pipeline
   ============================================================ */

test('applySearchFilters: default filters keep every property, ordered featured-first', () => {
  const result = applySearchFilters(inventory(), filters())
  assert.deepEqual(ids(result), [
    'p-home-1',
    'p-home-3',
    'p-home-2',
    'p-land-1',
    'p-home-4',
  ])
})

test('applySearchFilters: category land matches only land; homes excludes land', () => {
  assert.deepEqual(
    ids(applySearchFilters(inventory(), filters({ category: 'land' }))),
    ['p-land-1'],
  )
  assert.deepEqual(
    ids(applySearchFilters(inventory(), filters({ category: 'homes' }))),
    ['p-home-1', 'p-home-3', 'p-home-2', 'p-home-4'],
  )
})

test('applySearchFilters: maxPrice caps price and excludes price-less properties', () => {
  const capped = filters({ maxPrice: '2000000' })
  assert.deepEqual(ids(applySearchFilters(inventory(), capped)), [
    'p-home-3',
    'p-home-2',
    'p-land-1',
  ])
})

test('applySearchFilters: beds floor matches only homes with enough beds; land is excluded (mirrors the server SQL)', () => {
  // The server SQL and the client matcher agree: a beds filter only applies to
  // non-land inventory, and land rows are EXCLUDED when a beds floor is set.
  const threePlus = filters({ beds: '3' })
  assert.deepEqual(ids(applySearchFilters(inventory(), threePlus)), [
    'p-home-1', // 4 beds
    'p-home-3', // 3 beds
    'p-home-4', // 5 beds
  ])
})

test('applySearchFilters: view requires membership, case-insensitively', () => {
  const ocean = filters({ view: 'ocean' })
  assert.deepEqual(ids(applySearchFilters(inventory(), ocean)), [
    'p-home-1',
    'p-land-1',
  ])
  assert.deepEqual(
    ids(applySearchFilters(inventory(), filters({ view: 'Harbor' }))),
    [],
  )
})

test('applySearchFilters: free-text searches the canonical haystack case-insensitively', () => {
  const byName = filters({ q: '  CASA MAR  ' })
  assert.deepEqual(ids(applySearchFilters(inventory(), byName)), ['p-home-1'])

  const byLocation = filters({ q: 'flamenco' })
  assert.deepEqual(ids(applySearchFilters(inventory(), byLocation)), [
    'p-home-1',
    'p-home-3',
    'p-home-2',
    'p-land-1',
    'p-home-4',
  ])

  const byType = filters({ q: 'villa' })
  assert.deepEqual(ids(applySearchFilters(inventory(), byType)), [
    'p-home-2', // propertyType villa
    'p-home-4', // name "Villa Sin Precio"
  ])

  const byViewWord = filters({ q: 'beach' })
  assert.deepEqual(ids(applySearchFilters(inventory(), byViewWord)), [
    'p-home-3', // views ['Beach']
  ])
})

test('applySearchFilters: combined filters narrow jointly', () => {
  const combined = filters({
    category: 'homes',
    maxPrice: '3000000',
    beds: '3',
    view: 'Ocean',
  })
  assert.deepEqual(ids(applySearchFilters(inventory(), combined)), ['p-home-1'])
})

test('applySearchFilters: empty inventory yields an empty result', () => {
  assert.deepEqual(applySearchFilters([], filters()), [])
})

test('applySearchFilters: idempotent over an already-filtered list (server + client converge)', () => {
  // The server returns the SQL-filtered list; the showroom re-applies the
  // same filters client-side. The second pass must be a no-op for the match
  // set, so the client can never contradict the server.
  const structured = filters({ category: 'homes', maxPrice: '2500000' })
  const serverFiltered = applySearchFilters(inventory(), structured)
  const doubleFiltered = applySearchFilters(serverFiltered, structured)
  assert.deepEqual(ids(doubleFiltered), ids(serverFiltered))

  const narrowed = filters({ ...structured, beds: '3' })
  const narrowedAgain = applySearchFilters(
    applySearchFilters(serverFiltered, narrowed),
    narrowed,
  )
  assert.deepEqual(ids(narrowedAgain), ids(applySearchFilters(serverFiltered, narrowed)))
})

test('applySearchFilters: sort choice never changes which properties match', () => {
  const base = filters({ q: 'casa' })
  const matchedIds: Record<string, string[]> = {}
  for (const sort of ['featured', 'price-high', 'price-low', 'name'] as const) {
    matchedIds[sort] = [...ids(applySearchFilters(inventory(), { ...base, sort }))].sort()
  }
  const first = matchedIds.featured
  for (const sort of ['price-high', 'price-low', 'name'] as const) {
    assert.deepEqual(
      matchedIds[sort],
      first,
      `sort=${sort} must not change the matched id set`,
    )
  }
})

/* ============================================================
   searchParamsToFilters — inverse of searchFiltersToQuery
   ============================================================ */

test('searchParamsToFilters: empty params yield the default filters', () => {
  assert.deepEqual(searchParamsToFilters(new URLSearchParams()), filters())
})

test('searchParamsToFilters: maps every canonical param', () => {
  const parsed = searchParamsToFilters(
    new URLSearchParams(
      'category=homes&q=Beach&maxPrice=2000000&beds=3&view=Ocean&sort=price-high',
    ),
  )
  assert.deepEqual(parsed, {
    category: 'homes',
    q: 'Beach',
    maxPrice: '2000000',
    beds: '3',
    view: 'Ocean',
    sort: 'price-high',
  })
})

test('searchParamsToFilters: unknown category and sort fall back to defaults', () => {
  const parsed = searchParamsToFilters(
    new URLSearchParams('category=bogus&sort=random'),
  )
  assert.equal(parsed.category, 'all')
  assert.equal(parsed.sort, 'featured')
})

test('searchParamsToFilters: free-text is taken raw so the next push trims it', () => {
  const parsed = searchParamsToFilters(new URLSearchParams('q=%20%20casa%20%20'))
  assert.equal(parsed.q, '  casa  ')
})

test('searchParamsToFilters: first value wins for repeated params', () => {
  const parsed = searchParamsToFilters(
    new URLSearchParams('view=Ocean&view=Bay'),
  )
  assert.equal(parsed.view, 'Ocean')
})

test('URL round trip: filters -> query -> filters is lossless for canonical inputs', () => {
  const canonical = filters({
    category: 'homes',
    q: 'beach',
    maxPrice: '2000000',
    beds: '3',
    view: 'Ocean',
    sort: 'price-low',
  })
  const qs = searchFiltersToQuery(canonical)
  assert.deepEqual(searchParamsToFilters(new URLSearchParams(qs)), canonical)
})

test('URL round trip: query -> filters -> query is stable (the push-guard contract)', () => {
  // The showroom's push effect skips when the re-serialized query equals the
  // current URL; if this round trip ever produced a different string, the
  // effect would churn on every render.
  for (const qs of [
    '',
    'category=land',
    'q=Casa',
    'maxPrice=2000000&beds=3',
    'view=Ocean&sort=price-high',
    'category=homes&q=Beach&maxPrice=2000000&beds=3&view=Ocean&sort=name',
  ]) {
    const reparsed = searchFiltersToQuery(
      searchParamsToFilters(new URLSearchParams(qs)),
    )
    assert.equal(reparsed, qs, `round trip must be stable for ${qs || '(empty)'}`)
  }
})

test('URL round trip: whitespace-only free-text self-cleans on the next push', () => {
  const dirty = new URLSearchParams('q=%20%20')
  const reparsed = searchFiltersToQuery(searchParamsToFilters(dirty))
  assert.equal(reparsed, '')
})
