// ---------------------------------------------------------------------------
// TESTV2 UI tier — client CRM controller (glass-box).
//
// The MVI "LEGO" claim, proven: the ClientLensController is a framework-neutral
// pure class. We drive it here from `node:test` with a controllable fake
// `ClientLensSource` (no React, no DOM, no HTTP, no DB) and assert on the
// published PageModel. This is the same glass-box shape as the service tier —
// swap the fake for `HttpClientLensSource` and you get the black-box tier.
//
// These tests exercise the runtime concurrency semantics (latest-wins loads,
// serial paging/save, parallel fan-out) that otherwise only exist inside a
// running component.
// ---------------------------------------------------------------------------
import { test } from 'node:test'
import assert from 'node:assert/strict'

import { ClientLensController } from '../ui/client-lens/client-lens-controller'
import type { ClientLensSource } from '../ui/client-lens/source'
import type { Client } from '../lib/portal/types'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function client(id: string, displayName: string, notes = ''): Client {
  return { id, displayName, role: 'buyer', status: 'active', notes } as Client
}

/** A few microtask/setTimeout turns lets fire-and-forget lane dispatches settle. */
async function settle(turns = 3): Promise<void> {
  for (let i = 0; i < turns; i += 1) {
    await new Promise((r) => setTimeout(r, 0))
  }
}

type ListResult = { rows: Array<{ id: string }>; total: number; page: number; pageSize: number }
type ListProvider = (request: { query: string; page: number; pageSize: number }) => ListResult | Promise<ListResult>

type FakeSource = ClientLensSource & { saveCalls: number; listCalls: number }

/** Build a controllable fake source; each method can be overridden per test. */
function makeFake(over: Partial<ClientLensSource> = {}): FakeSource {
  const source: FakeSource = {
    saveCalls: 0,
    listCalls: 0,
    async loadList(request) {
      this.listCalls += 1
      const pageSize = Math.max(1, request.pageSize)
      const total = 2
      const page = Math.min(Math.max(1, request.page), Math.max(1, Math.ceil(total / pageSize)))
      return { rows: [{ id: 'a' }, { id: 'b' }], total, page, pageSize }
    },
    async loadClient() {
      return null
    },
    async loadChannels() {
      return []
    },
    async loadPropertyContext(id) {
      return { personId: id, properties: [], observedAddresses: [] }
    },
    async saveNotes() {
      this.saveCalls += 1
    },
    ...over,
  }
  return source
}

test('clientLens.load lists the rail, auto-selects the first client, and fans out Person/Property/channel lanes', async () => {
  const clients = new Map<string, Client>([
    ['a', client('a', 'Alice')],
    ['b', client('b', 'Bob', 'met at open house')],
  ])
  const fake = makeFake({
    loadList: async () => ({ rows: [{ id: 'a' }, { id: 'b' }], total: 2, page: 1, pageSize: 50 }),
    loadClient: async (id) => clients.get(id) ?? null,
    loadChannels: async (id) =>
      id === 'a'
        ? [{ source: 'gmail', channel: 'email', totalCount: 3, inboundCount: 2, outboundCount: 1 }]
        : [],
    loadPropertyContext: async (id) => ({
      personId: id,
      properties: [],
      observedAddresses: id === 'a' ? [{ address: { city: 'Culebra' } }] : [],
    }),
  })

  const controller = new ClientLensController(fake)
  await controller.dispatch({ operation: 'clientLens.load', payload: {} })
  await settle()

  const model = controller.snapshot()
  assert.equal(model.list.length, 2)
  assert.equal(model.total, 2)
  assert.equal(model.listLoading, false)
  assert.equal(model.listError, null)

  // Auto-selected the first row and loaded its lanes.
  assert.equal(model.selectedClientId, 'a')
  assert.equal(model.client?.displayName, 'Alice')
  assert.equal(model.clientLoading, false)
  assert.equal(model.clientError, null)

  // The six relationship slots are always present; gmail is connected for Alice.
  assert.equal(model.channels.length, 6)
  assert.equal(model.channels.find((s) => s.slot === 'gmail')?.connected, true)

  assert.ok(model.propertyContext, 'property lane loaded')
  assert.equal(model.propertyLoading, false)
})

test('a superseded list load cannot overwrite a newer response (latest-wins on the rail)', async () => {
  const first = deferred<ListResult>()
  const second = deferred<ListResult>()
  let call = 0
  const fake = makeFake({
    loadList: () => {
      call += 1
      return call === 1 ? first.promise : second.promise
    },
  })

  const controller = new ClientLensController(fake)
  // Fire both loads without awaiting the first: the first is superseded and held.
  void controller.dispatch({ operation: 'clientLens.load', payload: {} }) // call 1 (stale, held)
  const secondDone = controller.dispatch({ operation: 'clientLens.load', payload: {} }) // call 2 (current)

  // Newer response lands first.
  second.resolve({ rows: [{ id: 'new' }], total: 1, page: 1, pageSize: 50 })
  await secondDone
  await settle()
  assert.equal(controller.snapshot().list[0]?.id, 'new')
  assert.equal(controller.snapshot().total, 1)

  // Older response lands late: its model write must be ignored.
  first.resolve({ rows: [{ id: 'stale' }], total: 999, page: 1, pageSize: 50 })
  await settle()
  const model = controller.snapshot()
  assert.equal(model.list[0]?.id, 'new', 'stale response must not overwrite the rail')
  assert.equal(model.total, 1, 'stale total must be ignored')
  assert.equal(model.listLoading, false)
  assert.equal(model.listError, null)
})

test('selectClient switches selection and ignores late lane writes from a stale selection', async () => {
  const aliceClient = deferred<Client | null>()
  const clients = new Map<string, Client>([
    ['a', client('a', 'Alice')],
    ['b', client('b', 'Bob')],
  ])
  const fake = makeFake({
    loadClient: (id) => (id === 'a' ? aliceClient.promise : Promise.resolve(clients.get('b') ?? null)),
  })

  const controller = new ClientLensController(fake)
  await controller.dispatch({ operation: 'clientLens.selectClient', payload: { personId: 'a' } })
  // Alice's Person lane is still in flight.

  await controller.dispatch({ operation: 'clientLens.selectClient', payload: { personId: 'b' } })
  await settle()
  assert.equal(controller.snapshot().selectedClientId, 'b')
  assert.equal(controller.snapshot().client?.displayName, 'Bob')

  // Alice's stale Person lane resolves last — the guard must drop it.
  aliceClient.resolve(client('a', 'Alice'))
  await settle()
  const model = controller.snapshot()
  assert.equal(model.selectedClientId, 'b')
  assert.equal(model.client?.displayName, 'Bob', 'late write from stale selection must be ignored')
  assert.equal(model.clientLoading, false)
})


test('a failed client lane reports an error without breaking the selection', async () => {
  const fake = makeFake({
    loadClient: async () => {
      throw new Error('detail service down')
    },
  })
  const controller = new ClientLensController(fake)
  await controller.dispatch({ operation: 'clientLens.selectClient', payload: { personId: 'a' } })
  await settle()

  const model = controller.snapshot()
  assert.equal(model.selectedClientId, 'a')
  assert.equal(model.client, null)
  assert.equal(model.clientLoading, false)
  assert.equal(model.clientError, 'detail service down')
})

test('notes: a changed draft saves, the clean save is a no-op against the source, failure surfaces', async () => {
  const clients = new Map<string, Client>([['a', client('a', 'Alice')]])
  const fake = makeFake({
    loadClient: async (id) => clients.get(id) ?? null,
  })

  const controller = new ClientLensController(fake)
  await controller.dispatch({ operation: 'clientLens.selectClient', payload: { personId: 'a' } })
  await settle()

  await controller.dispatch({ operation: 'clientLens.notesChanged', payload: { notes: 'prefers weekends' } })
  assert.equal(controller.snapshot().notesDraft, 'prefers weekends')

  await controller.dispatch({ operation: 'clientLens.saveNotes', payload: {} })
  await settle()
  assert.equal(controller.snapshot().notesSaved, 'prefers weekends')
  assert.equal(controller.snapshot().notesStatus, 'Saved')
  assert.equal(controller.snapshot().client?.notes, 'prefers weekends')

  // Draft now equals saved => a second save must not hit the source.
  await controller.dispatch({ operation: 'clientLens.saveNotes', payload: {} })
  await settle()
  assert.equal(fake.saveCalls, 1, 'clean save must not call the source again')
})

test('a failed save surfaces notesStatus without corrupting the draft', async () => {
  const fake = makeFake({
    saveNotes: async () => {
      throw new Error('could not write notes')
    },
  })
  const controller = new ClientLensController(fake)
  await controller.dispatch({ operation: 'clientLens.selectClient', payload: { personId: 'a' } })
  await settle()
  await controller.dispatch({ operation: 'clientLens.notesChanged', payload: { notes: 'draft text' } })

  await controller.dispatch({ operation: 'clientLens.saveNotes', payload: {} })
  await settle()
  const model = controller.snapshot()
  assert.equal(model.notesStatus, 'could not write notes')
  assert.equal(model.notesDraft, 'draft text')
  assert.equal(model.notesSaving, false)
})

test('pagination clamps at page 1 and queryChanged resets page to 1', async () => {
  const fake = makeFake()
  const controller = new ClientLensController(fake)
  await controller.dispatch({ operation: 'clientLens.load', payload: {} })
  await settle()
  assert.equal(controller.snapshot().page, 1)

  // previousPage clamps at 1 (still triggers a reload, stays at 1).
  await controller.dispatch({ operation: 'clientLens.previousPage', payload: {} })
  await settle()
  assert.equal(controller.snapshot().page, 1)

  // A new query resets to page 1 immediately (no source round-trip needed).
  await controller.dispatch({ operation: 'clientLens.queryChanged', payload: { query: 'ali' } })
  assert.equal(controller.snapshot().query, 'ali')
  assert.equal(controller.snapshot().page, 1)
})

