import { after, before, test } from 'node:test'
import assert from 'node:assert/strict'

// PX-22 Favorites — browser-local storage seam (zero Neon, zero React).
// The seam is `lib/favorites.ts`: canonical-id keyed, deduped, stale-pruned
// against the live public set, backward compatible with the legacy
// array-of-ids format, and cross-component synced via a custom event.
// These tests prove the contract against a stub `window.localStorage`
// EventTarget, including the no-op-write guard that keeps event listeners
// from re-triggering themselves.

import {
  FAVORITES_CHANGED_EVENT,
  isFavorite,
  pruneFavorites,
  readFavorites,
  removeFavorite,
  toggleFavorite,
  writeFavorites,
} from '../../lib/favorites'
import type { SavedPropertyEntry } from '../../lib/favorites'

type EventListener = (event: { type: string }) => void

const entry = (
  id: string,
  slug = `${id}-slug`,
  name = `Property ${id}`,
): SavedPropertyEntry => ({ id, slug, name })

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

test('readFavorites returns [] when nothing is stored', () => {
  store.clear()
  assert.deepEqual(readFavorites(), [])
})

test('readFavorites returns [] for corrupted JSON and non-array payloads', () => {
  store.set('culebraluxe:saved-properties', 'not-json{')
  assert.deepEqual(readFavorites(), [])

  store.set('culebraluxe:saved-properties', JSON.stringify({ id: 'x' }))
  assert.deepEqual(readFavorites(), [])
})

test('readFavorites is backward compatible with the legacy array-of-ids format', () => {
  store.set('culebraluxe:saved-properties', JSON.stringify(['a', '', 'b']))
  assert.deepEqual(readFavorites(), [
    { id: 'a', slug: '', name: '' },
    { id: 'b', slug: '', name: '' },
  ])
})

test('readFavorites filters malformed entries and keeps valid ones', () => {
  store.set(
    'culebraluxe:saved-properties',
    JSON.stringify([
      entry('a'),
      { id: 'b' },
      { id: 7, slug: 's', name: 'n' },
      { slug: 'c', name: 'C' },
      null,
      entry('d'),
    ]),
  )
  assert.deepEqual(readFavorites(), [entry('a'), entry('d')])
})

test('writeFavorites persists entries and identical content is a no-op without dispatch', () => {
  store.clear()
  dispatchCalls.length = 0

  writeFavorites([entry('1'), entry('2')])
  assert.equal(dispatchCalls.length, 1)
  assert.equal(dispatchCalls[0], FAVORITES_CHANGED_EVENT)

  writeFavorites([entry('1'), entry('2')])
  assert.equal(dispatchCalls.length, 1, 'identical write must not dispatch again')
  assert.deepEqual(readFavorites(), [entry('1'), entry('2')])
})

test('toggleFavorite adds an entry and returns true', () => {
  store.clear()
  dispatchCalls.length = 0
  assert.equal(toggleFavorite(entry('a')), true)
  assert.deepEqual(readFavorites(), [entry('a')])
  assert.deepEqual(dispatchCalls, [FAVORITES_CHANGED_EVENT])
})

test('toggleFavorite removes an existing entry by canonical id and returns false', () => {
  store.clear()
  toggleFavorite(entry('a'))
  dispatchCalls.length = 0

  assert.equal(toggleFavorite(entry('a')), false)
  assert.deepEqual(readFavorites(), [])
  assert.equal(dispatchCalls.length, 1)
})

test('toggleFavorite dedupes by id, not by slug or name', () => {
  store.clear()
  toggleFavorite(entry('a', 'first-slug', 'First Name'))
  assert.equal(
    toggleFavorite({ id: 'a', slug: 'renamed-slug', name: 'Renamed' }),
    false,
  )
  assert.deepEqual(readFavorites(), [])
})

test('isFavorite reflects the persisted state', () => {
  store.clear()
  assert.equal(isFavorite('a'), false)
  toggleFavorite(entry('a'))
  assert.equal(isFavorite('a'), true)
  assert.equal(isFavorite('missing'), false)
})

test('removeFavorite removes by canonical id and returns whether it was present', () => {
  store.clear()
  toggleFavorite(entry('1'))
  toggleFavorite(entry('2'))
  dispatchCalls.length = 0

  assert.equal(removeFavorite('1'), true)
  assert.deepEqual(readFavorites(), [entry('2')])
  assert.deepEqual(dispatchCalls, [FAVORITES_CHANGED_EVENT])

  assert.equal(removeFavorite('missing'), false)
  assert.deepEqual(readFavorites(), [entry('2')])
  assert.equal(dispatchCalls.length, 1, 'removing a missing id must not dispatch')
})

test('pruneFavorites drops stale entries by id or slug, persists, and returns survivors', () => {
  store.clear()
  toggleFavorite(entry('a', 'live-a'))
  toggleFavorite(entry('b', 'gone-b'))
  toggleFavorite(entry('c', 'live-c'))
  // Legacy entry: id-only, no slug — survives by canonical id.
  toggleFavorite({ id: 'd', slug: '', name: 'Legacy D' })

  const survivors = pruneFavorites(['a', 'd'], ['live-a', 'live-c'])
  assert.deepEqual(survivors, [entry('a', 'live-a'), entry('c', 'live-c'), { id: 'd', slug: '', name: 'Legacy D' }])
  assert.deepEqual(readFavorites(), survivors)
})

test('pruneFavorites with an empty live set clears the saved list', () => {
  store.clear()
  toggleFavorite(entry('a', 'old-a'))
  toggleFavorite(entry('b', 'old-b'))

  assert.deepEqual(pruneFavorites([], []), [])
  assert.deepEqual(readFavorites(), [])
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

  assert.deepEqual(readFavorites(), [])
  assert.doesNotThrow(() => toggleFavorite(entry('a')))
  assert.doesNotThrow(() => writeFavorites([entry('a')]))
  assert.equal(toggleFavorite(entry('a')), false, 'failed write reports unsaved')

  installWindowStub()
})

test('event listeners converge: a real change dispatches once, a refresh no-op does not re-dispatch', () => {
  store.clear()
  dispatchCalls.length = 0
  let syncCount = 0
  const listener = () => {
    syncCount += 1
    pruneFavorites(['a', 'b'], ['live-1', 'live-2'])
  }
  globalThis.window.addEventListener(FAVORITES_CHANGED_EVENT, listener)

  toggleFavorite(entry('1', 'live-1'))
  toggleFavorite(entry('2', 'live-2'))

  // Two real toggles -> two events; the listener's prune is a no-op write
  // that must not loop back into further dispatches.
  assert.equal(syncCount, 2)
  assert.deepEqual(readFavorites(), [entry('1', 'live-1'), entry('2', 'live-2')])
  assert.equal(dispatchCalls.length, 2)

  globalThis.window.removeEventListener(FAVORITES_CHANGED_EVENT, listener)
})

test('server-side (no window) calls are safe and read empty', () => {
  removeWindowStub()
  try {
    assert.deepEqual(readFavorites(), [])
    assert.equal(isFavorite('a'), false)
    assert.equal(toggleFavorite(entry('a')), false)
    assert.equal(removeFavorite('a'), false)
    assert.doesNotThrow(() => writeFavorites([entry('a')]))
    assert.doesNotThrow(() => pruneFavorites(['a'], ['a-slug']))
  } finally {
    installWindowStub()
  }
})
