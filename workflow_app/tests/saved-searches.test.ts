import { after, before, test } from 'node:test'
import assert from 'node:assert/strict'

// PX-23 Saved Searches + Alerts — browser-local V1 (zero Neon, zero React).
// Two pure seams are proven here:
//
//   lib/search-contract.ts — the canonical buyers search contract: the
//     SearchFilters shape, the canonical dedupe signature, the apply-URL
//     builder, the human label, and the client-side alert matcher that
//     mirrors the server filter SQL (the server stays authoritative for what
//     renders).
//
//   lib/saved-searches.ts — the storage seam: dedupe by filter signature,
//     remove, mark-viewed, and the honest "new matches since you last viewed"
//     diff that powers the alerts.
//
// Storage tests run against a stub `window.localStorage` EventTarget,
// including the no-op-write guard that keeps event listeners from
// re-triggering themselves.

import type { PropertySummary } from '../../db/properties'
import {
  DEFAULT_SEARCH_FILTERS,
  describeSearchFilters,
  matchesSearchFilters,
  searchFiltersToKey,
  searchFiltersToQuery,
} from '../../lib/search-contract'
import type { SearchFilters } from '../../lib/search-contract'
import {
  SAVED_SEARCHES_CHANGED_EVENT,
  markSavedSearchViewed,
  newMatchIds,
  readSavedSearches,
  removeSavedSearch,
  saveSearch,
  writeSavedSearches,
} from '../../lib/saved-searches'
import type { SavedSearch } from '../../lib/saved-searches'

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

/* ============================================================
   Search contract — pure (no window needed)
   ============================================================ */

test('matchesSearchFilters: default filters match every property', () => {
  const land = property({ propertyType: 'land', listPrice: null })
  const house = property()
  assert.equal(matchesSearchFilters(land, filters()), true)
  assert.equal(matchesSearchFilters(house, filters()), true)
})

test('matchesSearchFilters: category land matches only land', () => {
  const land = property({ propertyType: 'land' })
  const house = property({ propertyType: 'house' })
  assert.equal(matchesSearchFilters(land, filters({ category: 'land' })), true)
  assert.equal(
    matchesSearchFilters(house, filters({ category: 'land' })),
    false,
  )
})

test('matchesSearchFilters: category homes excludes land', () => {
  const land = property({ propertyType: 'land' })
  const house = property({ propertyType: 'house' })
  assert.equal(matchesSearchFilters(house, filters({ category: 'homes' })), true)
  assert.equal(
    matchesSearchFilters(land, filters({ category: 'homes' })),
    false,
  )
})

test('matchesSearchFilters: maxPrice caps list price and never matches price-less properties', () => {
  const capped = filters({ maxPrice: '2000000' })
  assert.equal(matchesSearchFilters(property({ listPrice: 1_500_000 }), capped), true)
  assert.equal(matchesSearchFilters(property({ listPrice: 2_500_000 }), capped), false)
  assert.equal(matchesSearchFilters(property({ listPrice: null }), capped), false)
  assert.equal(
    matchesSearchFilters(
      property({ listPrice: 9_000_000 }),
      filters({ maxPrice: '' }),
    ),
    true,
  )
})

test('matchesSearchFilters: beds floor never applies to land and excludes smaller homes', () => {
  const threePlus = filters({ beds: '3' })
  assert.equal(matchesSearchFilters(property({ bedrooms: 3 }), threePlus), true)
  assert.equal(matchesSearchFilters(property({ bedrooms: 2 }), threePlus), false)
  assert.equal(
    matchesSearchFilters(property({ bedrooms: null }), threePlus),
    false,
  )
  assert.equal(
    matchesSearchFilters(
      property({ propertyType: 'land', bedrooms: null }),
      threePlus,
    ),
    false,
  )
  assert.equal(
    matchesSearchFilters(property({ bedrooms: 5 }), filters({ beds: '' })),
    true,
  )
})

test('matchesSearchFilters: view requires membership, case-insensitively', () => {
  const ocean = filters({ view: 'ocean' })
  assert.equal(matchesSearchFilters(property({ views: ['Ocean'] }), ocean), true)
  assert.equal(
    matchesSearchFilters(property({ views: ['Bay'] }), ocean),
    false,
  )
  assert.equal(
    matchesSearchFilters(property({ views: [] }), filters({ view: '' })),
    true,
  )
})

test('matchesSearchFilters: free-text searches the haystack case-insensitively', () => {
  const byLocation = filters({ q: 'flamenco' })
  assert.equal(matchesSearchFilters(property({ location: 'Flamenco' }), byLocation), true)
  assert.equal(
    matchesSearchFilters(property({ location: 'Ensenada' }), byLocation),
    false,
  )

  const byViewWord = filters({ q: 'ocean' })
  assert.equal(matchesSearchFilters(property({ views: ['Ocean'] }), byViewWord), true)
  assert.equal(
    matchesSearchFilters(property({ views: ['Sunset'] }), byViewWord),
    false,
  )

  const byName = filters({ q: '  CASA  ' })
  assert.equal(matchesSearchFilters(property({ name: 'Casa Test' }), byName), true)
})

test('matchesSearchFilters: sort never affects matching', () => {
  const base = filters({ q: 'casa' })
  for (const sort of ['featured', 'price-high', 'price-low', 'name'] as const) {
    assert.equal(
      matchesSearchFilters(property(), { ...base, sort }),
      true,
      `sort=${sort} must not change the match result`,
    )
  }
})

test('searchFiltersToKey: canonical across construction, whitespace, and case', () => {
  const a = filters({
    category: 'homes',
    q: '  Beach ',
    maxPrice: '2000000',
    view: 'OCEAN',
    sort: 'price-high',
  })
  const b = filters({
    category: 'homes',
    q: 'beach',
    maxPrice: ' 2000000 ',
    view: 'ocean',
    sort: 'price-high',
  })
  assert.equal(searchFiltersToKey(a), searchFiltersToKey(b))

  assert.notEqual(
    searchFiltersToKey(filters({ category: 'homes' })),
    searchFiltersToKey(filters({ category: 'all' })),
  )
  assert.notEqual(
    searchFiltersToKey(filters({ maxPrice: '2000000' })),
    searchFiltersToKey(filters({ maxPrice: '3000000' })),
  )
})

test('searchFiltersToQuery: mirrors the URL contract, omitting defaults', () => {
  assert.equal(searchFiltersToQuery(filters()), '')

  assert.equal(
    searchFiltersToQuery(
      filters({
        category: 'homes',
        q: '  Beach  ',
        maxPrice: '2000000',
        beds: '3',
        view: 'Ocean',
        sort: 'price-high',
      }),
    ),
    'category=homes&q=Beach&maxPrice=2000000&beds=3&view=Ocean&sort=price-high',
  )

  assert.equal(
    searchFiltersToQuery(filters({ sort: 'price-low' })),
    'sort=price-low',
  )
})

test('describeSearchFilters: names from non-default parts only', () => {
  assert.equal(describeSearchFilters(filters()), 'All properties')
  assert.equal(
    describeSearchFilters(
      filters({
        category: 'homes',
        maxPrice: '2000000',
        beds: '3',
        view: 'Ocean',
        q: 'beach',
      }),
    ),
    'Homes & Villas · up to $2,000,000 · 3+ beds · Ocean view · “beach”',
  )
  assert.equal(
    describeSearchFilters(filters({ category: 'land', sort: 'price-low' })),
    'Land',
  )
})

/* ============================================================
   Storage seam — window stub
   ============================================================ */

type EventListener = (event: { type: string }) => void

let store: Map<string, string>
let listeners: Map<string, Set<EventListener>>
let dispatchCalls: string[]

function installWindowStub() {
  store = new Map()
  listeners = new Map()
  dispatchCalls = []

  const windowStub = {
    localStorage: {
      getItem: (key: string) => (store.has(key) ? store.get(key)! : null),
      setItem: (key: string, value: string) => {
        store.set(key, value)
      },
      removeItem: (key: string) => {
        store.delete(key)
      },
      clear: () => {
        store.clear()
      },
    },
    dispatchEvent: (event: { type: string }) => {
      dispatchCalls.push(event.type)
      for (const listener of listeners.get(event.type) ?? []) {
        listener(event)
      }
      return true
    },
    addEventListener: (type: string, listener: EventListener) => {
      if (!listeners.has(type)) listeners.set(type, new Set())
      listeners.get(type)!.add(listener)
    },
    removeEventListener: (type: string, listener: EventListener) => {
      listeners.get(type)?.delete(listener)
    },
  }

  Object.defineProperty(globalThis, 'window', {
    value: windowStub,
    configurable: true,
    writable: true,
  })
}

function removeWindowStub() {
  delete (globalThis as { window?: unknown }).window
}

before(() => {
  installWindowStub()
})

after(() => {
  removeWindowStub()
})

const STORAGE_KEY = 'culebraluxe:saved-searches'

const seedEntry = (overrides: Partial<SavedSearch> = {}): SavedSearch => ({
  id: 'ss-1',
  name: 'Homes',
  filters: filters({ category: 'homes' }),
  createdAt: '2026-08-22T00:00:00.000Z',
  lastCheckedAt: '2026-08-22T00:00:00.000Z',
  lastMatchIds: ['p-1', 'p-2'],
  ...overrides,
})

test('readSavedSearches returns [] when nothing is stored or payloads are malformed', () => {
  store.clear()
  assert.deepEqual(readSavedSearches(), [])

  store.set(STORAGE_KEY, 'not-json{')
  assert.deepEqual(readSavedSearches(), [])

  store.set(STORAGE_KEY, JSON.stringify({ id: 'x' }))
  assert.deepEqual(readSavedSearches(), [])
})

test('readSavedSearches filters malformed entries and keeps valid ones', () => {
  store.set(
    STORAGE_KEY,
    JSON.stringify([
      seedEntry(),
      { id: 'ss-2' },
      seedEntry({ id: 'ss-3', filters: { category: 'nope', q: 7 } }),
      null,
      seedEntry({ id: 'ss-4', lastMatchIds: ['ok', 5] }),
    ]),
  )
  const result = readSavedSearches()
  assert.equal(result.length, 1)
  assert.equal(result[0].id, 'ss-1')
})

test('writeSavedSearches persists and identical content is a no-op without dispatch', () => {
  store.clear()
  dispatchCalls.length = 0

  writeSavedSearches([seedEntry()])
  assert.equal(dispatchCalls.length, 1)
  assert.equal(dispatchCalls[0], SAVED_SEARCHES_CHANGED_EVENT)

  writeSavedSearches([seedEntry()])
  assert.equal(dispatchCalls.length, 1, 'identical write must not dispatch again')
  assert.deepEqual(readSavedSearches(), [seedEntry()])
})

test('saveSearch creates an entry with the seen set seeded from the current matches', () => {
  store.clear()
  dispatchCalls.length = 0

  const result = saveSearch({
    name: 'Homes & Villas',
    filters: filters({ category: 'homes' }),
    initialMatchIds: ['p-1', 'p-2'],
  })

  assert.equal(result.created, true)
  const saved = readSavedSearches()
  assert.equal(saved.length, 1)
  assert.equal(saved[0].id, result.id)
  assert.equal(saved[0].name, 'Homes & Villas')
  assert.deepEqual(saved[0].filters, filters({ category: 'homes' }))
  assert.deepEqual(saved[0].lastMatchIds, ['p-1', 'p-2'])
  assert.equal(typeof saved[0].createdAt, 'string')
  assert.equal(saved[0].lastCheckedAt, saved[0].createdAt)
  assert.deepEqual(dispatchCalls, [SAVED_SEARCHES_CHANGED_EVENT])
})

test('saveSearch dedupes by filter signature and refreshes the existing entry', () => {
  store.clear()
  saveSearch({
    name: 'First',
    filters: filters({ category: 'homes', q: '  beach  ', view: '  Ocean  ' }),
    initialMatchIds: ['p-1'],
  })

  const refreshed = saveSearch({
    name: 'Second',
    // Logically identical (q trim/lowercase + view trim/case): same signature.
    filters: filters({ category: 'homes', q: 'beach', view: 'ocean' }),
    initialMatchIds: ['p-1', 'p-9'],
  })

  assert.equal(refreshed.created, false)
  const saved = readSavedSearches()
  assert.equal(saved.length, 1, 're-saving an identical search must not duplicate')
  assert.equal(saved[0].name, 'Second')
  assert.deepEqual(saved[0].lastMatchIds, ['p-1', 'p-9'])
})

test('saveSearch stores distinct filter sets as distinct entries', () => {
  store.clear()
  saveSearch({ name: 'Homes', filters: filters({ category: 'homes' }) })
  saveSearch({ name: 'Land', filters: filters({ category: 'land' }) })
  saveSearch({ name: 'All', filters: filters() })

  assert.equal(readSavedSearches().length, 3)
})

test('removeSavedSearch removes by id and reports whether it was present', () => {
  store.clear()
  const first = saveSearch({ name: 'Homes', filters: filters({ category: 'homes' }) })
  saveSearch({ name: 'Land', filters: filters({ category: 'land' }) })
  dispatchCalls.length = 0

  assert.equal(removeSavedSearch(first.id), true)
  assert.equal(readSavedSearches().length, 1)
  assert.deepEqual(dispatchCalls, [SAVED_SEARCHES_CHANGED_EVENT])

  assert.equal(removeSavedSearch('missing'), false)
  assert.equal(readSavedSearches().length, 1)
  assert.equal(dispatchCalls.length, 1, 'removing a missing id must not dispatch')
})

test('markSavedSearchViewed records the seen set and a timestamp', () => {
  store.clear()
  const created = saveSearch({
    name: 'Homes',
    filters: filters({ category: 'homes' }),
    initialMatchIds: ['p-1'],
  })
  dispatchCalls.length = 0

  assert.equal(markSavedSearchViewed(created.id, ['p-1', 'p-2', 'p-3']), true)
  const saved = readSavedSearches()
  assert.deepEqual(saved[0].lastMatchIds, ['p-1', 'p-2', 'p-3'])
  assert.equal(typeof saved[0].lastCheckedAt, 'string')
  assert.deepEqual(dispatchCalls, [SAVED_SEARCHES_CHANGED_EVENT])

  assert.equal(markSavedSearchViewed('missing', []), false)
  assert.equal(dispatchCalls.length, 1, 'a missing id must not dispatch')
})

test('newMatchIds diffs current matches against the seen set', () => {
  const search = seedEntry({ lastMatchIds: ['p-1', 'p-2'] })

  assert.deepEqual(newMatchIds(search, ['p-1', 'p-2', 'p-3']), ['p-3'])
  assert.deepEqual(newMatchIds(search, ['p-1']), [])
  assert.deepEqual(newMatchIds(search, ['p-9']), ['p-9'])

  // A property that newly matches (e.g. after a price drop into range) is a
  // new match; after it is viewed it stops alerting.
  const afterView = { ...search, lastMatchIds: ['p-1', 'p-2', 'p-9'] }
  assert.deepEqual(newMatchIds(afterView, ['p-1', 'p-2', 'p-9']), [])
})

test('event listeners converge: a real change dispatches once, a refresh no-op does not re-dispatch', () => {
  store.clear()
  dispatchCalls.length = 0
  let syncCount = 0
  const listener = () => {
    syncCount += 1
    // Refresh from storage inside the listener; a no-op write must not loop.
    readSavedSearches()
  }
  globalThis.window.addEventListener(SAVED_SEARCHES_CHANGED_EVENT, listener)

  saveSearch({ name: 'Homes', filters: filters({ category: 'homes' }) })
  saveSearch({ name: 'Land', filters: filters({ category: 'land' }) })

  assert.equal(syncCount, 2)
  assert.equal(dispatchCalls.length, 2)
  assert.equal(readSavedSearches().length, 2)

  globalThis.window.removeEventListener(SAVED_SEARCHES_CHANGED_EVENT, listener)
})

test('storage failures are contained (private mode): reads are empty, writes are ignored', () => {
  store.clear()
  Object.defineProperty(globalThis.window, 'localStorage', {
    value: {
      getItem: () => {
        throw new Error('denied')
      },
      setItem: () => {
        throw new Error('denied')
      },
    },
    configurable: true,
  })

  assert.deepEqual(readSavedSearches(), [])
  assert.doesNotThrow(() =>
    saveSearch({ name: 'Homes', filters: filters({ category: 'homes' }) }),
  )
  assert.doesNotThrow(() => writeSavedSearches([seedEntry()]))
  assert.doesNotThrow(() => removeSavedSearch('ss-1'))
  assert.doesNotThrow(() => markSavedSearchViewed('ss-1', []))

  installWindowStub()
})

test('server-side (no window) calls are safe and read empty', () => {
  removeWindowStub()
  try {
    assert.deepEqual(readSavedSearches(), [])
    const created = saveSearch({ name: 'Homes', filters: filters({ category: 'homes' }) })
    assert.equal(created.created, false, 'server-side save must not persist')
    assert.equal(removeSavedSearch('ss-1'), false)
    assert.equal(markSavedSearchViewed('ss-1', []), false)
    assert.doesNotThrow(() => writeSavedSearches([seedEntry()]))
  } finally {
    installWindowStub()
  }
})
