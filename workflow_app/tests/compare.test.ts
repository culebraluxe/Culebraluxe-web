import { after, before, test } from 'node:test'
import assert from 'node:assert/strict'

// PX-21 Compare Properties — browser-local storage seam (zero Neon, zero
// React). The seam is `lib/compare.ts`: canonical-id keyed, bounded to 3,
// deduped, stale-pruned, and cross-component synced via a custom event.
// These tests prove the contract against a stub `window.localStorage`
// EventTarget, including the capacity semantics (adding past the bound is
// rejected, never a silent drop) and the no-op-write guard that keeps the
// event listeners from re-triggering themselves.

import {
  COMPARE_CHANGED_EVENT,
  COMPARE_MAX,
  pruneCompare,
  readCompare,
  removeCompare,
  toggleCompare,
  writeCompare,
} from '../../lib/compare'
import type { CompareEntry } from '../../lib/compare'

type EventListener = (event: { type: string }) => void

const entry = (id: string, slug = `${id}-slug`, name = `Property ${id}`): CompareEntry => ({
  id,
  slug,
  name,
})

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

test('readCompare returns [] when nothing is stored', () => {
  store.clear()
  assert.deepEqual(readCompare(), [])
})

test('readCompare returns [] for corrupted JSON and non-array payloads', () => {
  store.set('culebraluxe:compare-properties', 'not-json{')
  assert.deepEqual(readCompare(), [])

  store.set('culebraluxe:compare-properties', JSON.stringify({ id: 'x' }))
  assert.deepEqual(readCompare(), [])
})

test('readCompare filters malformed entries and keeps valid ones', () => {
  store.set(
    'culebraluxe:compare-properties',
    JSON.stringify([
      entry('a'),
      { id: 'b' },
      { id: 7, slug: 's', name: 'n' },
      { slug: 'c', name: 'C' },
      null,
      entry('d'),
    ]),
  )
  assert.deepEqual(readCompare(), [entry('a'), entry('d')])
})

test('writeCompare persists entries bounded to COMPARE_MAX', () => {
  store.clear()
  writeCompare([entry('1'), entry('2'), entry('3'), entry('4'), entry('5')])
  assert.deepEqual(readCompare(), [entry('1'), entry('2'), entry('3')])
  assert.equal(readCompare().length, COMPARE_MAX)
})

test('writeCompare with unchanged content is a no-op and does not dispatch', () => {
  store.clear()
  dispatchCalls.length = 0

  writeCompare([entry('1'), entry('2')])
  assert.equal(dispatchCalls.length, 1)
  assert.equal(dispatchCalls[0], COMPARE_CHANGED_EVENT)

  writeCompare([entry('1'), entry('2')])
  assert.equal(dispatchCalls.length, 1, 'identical write must not dispatch again')
  assert.deepEqual(readCompare(), [entry('1'), entry('2')])
})

test('toggleCompare adds an entry and returns true', () => {
  store.clear()
  dispatchCalls.length = 0
  assert.equal(toggleCompare(entry('a')), true)
  assert.deepEqual(readCompare(), [entry('a')])
  assert.deepEqual(dispatchCalls, [COMPARE_CHANGED_EVENT])
})

test('toggleCompare removes an existing entry by canonical id and returns false', () => {
  store.clear()
  toggleCompare(entry('a'))
  dispatchCalls.length = 0

  assert.equal(toggleCompare(entry('a')), false)
  assert.deepEqual(readCompare(), [])
  assert.equal(dispatchCalls.length, 1)
})

test('toggleCompare dedupes by id, not by slug or name', () => {
  store.clear()
  toggleCompare(entry('a', 'first-slug', 'First Name'))
  assert.equal(toggleCompare({ id: 'a', slug: 'renamed-slug', name: 'Renamed' }), false)
  assert.deepEqual(readCompare(), [])
})

test('toggleCompare rejects an add past COMPARE_MAX without dropping the newest entry', () => {
  store.clear()
  dispatchCalls.length = 0
  assert.equal(toggleCompare(entry('1')), true)
  assert.equal(toggleCompare(entry('2')), true)
  assert.equal(toggleCompare(entry('3')), true)
  assert.equal(dispatchCalls.length, 3)

  // Fourth distinct property: rejected (returns false), nothing persisted,
  // no event — the stored selection and the button state stay in agreement.
  assert.equal(toggleCompare(entry('4')), false)
  assert.deepEqual(readCompare(), [entry('1'), entry('2'), entry('3')])
  assert.equal(dispatchCalls.length, 3)
})

test('removeCompare removes by canonical id and returns whether it was present', () => {
  store.clear()
  toggleCompare(entry('1'))
  toggleCompare(entry('2'))
  dispatchCalls.length = 0

  assert.equal(removeCompare('1'), true)
  assert.deepEqual(readCompare(), [entry('2')])
  assert.deepEqual(dispatchCalls, [COMPARE_CHANGED_EVENT])

  assert.equal(removeCompare('missing'), false)
  assert.deepEqual(readCompare(), [entry('2')])
  assert.equal(dispatchCalls.length, 1, 'removing a missing id must not dispatch')
})

test('pruneCompare drops stale slugs, persists the pruned list, and returns survivors', () => {
  store.clear()
  toggleCompare(entry('a', 'live-a'))
  toggleCompare(entry('b', 'gone-b'))
  toggleCompare(entry('c', 'live-c'))

  const survivors = pruneCompare(['live-a', 'live-c'])
  assert.deepEqual(survivors, [entry('a', 'live-a'), entry('c', 'live-c')])
  assert.deepEqual(readCompare(), [entry('a', 'live-a'), entry('c', 'live-c')])
})

test('pruneCompare frees slots when everything is stale', () => {
  store.clear()
  toggleCompare(entry('a', 'old-a'))
  toggleCompare(entry('b', 'old-b'))

  assert.deepEqual(pruneCompare(['nothing-live']), [])
  assert.deepEqual(readCompare(), [])
  assert.equal(toggleCompare(entry('fresh')), true, 'freed slot accepts a new entry')
})

test('storage failures are contained (private mode): reads are empty, writes are ignored', () => {
  store.clear()
  const originalGet = store.get.bind(store)
  const originalSet = store.set.bind(store)
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

  assert.deepEqual(readCompare(), [])
  assert.doesNotThrow(() => toggleCompare(entry('a')))
  assert.doesNotThrow(() => writeCompare([entry('a')]))

  Object.defineProperty(globalThis.window, 'localStorage', {
    value: {
      getItem: (key: string) => (store.has(key) ? store.get(key)! : null),
      setItem: (key: string, value: string) => {
        store.set(key, value)
      },
    },
    configurable: true,
  })
  void originalGet
  void originalSet
})

test('event listeners converge: a real change dispatches once, a refresh no-op does not re-dispatch', () => {
  store.clear()
  dispatchCalls.length = 0
  let syncCount = 0
  const listener = () => {
    syncCount += 1
    pruneCompare(['live-1', 'live-2'])
  }
  globalThis.window.addEventListener(COMPARE_CHANGED_EVENT, listener)

  toggleCompare(entry('1', 'live-1'))
  toggleCompare(entry('2', 'live-2'))

  // Two real toggles -> two events; the listener's prune is a no-op write
  // that must not loop back into further dispatches.
  assert.equal(syncCount, 2)
  assert.deepEqual(readCompare(), [entry('1', 'live-1'), entry('2', 'live-2')])
  assert.equal(dispatchCalls.length, 2)

  globalThis.window.removeEventListener(COMPARE_CHANGED_EVENT, listener)
})

test('server-side (no window) calls are safe and read empty', () => {
  removeWindowStub()
  try {
    assert.deepEqual(readCompare(), [])
    assert.equal(toggleCompare(entry('a')), false)
    assert.equal(removeCompare('a'), false)
    assert.doesNotThrow(() => writeCompare([entry('a')]))
    assert.doesNotThrow(() => pruneCompare(['a']))
  } finally {
    installWindowStub()
  }
})
