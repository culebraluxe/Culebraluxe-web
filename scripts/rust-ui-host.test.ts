// Tests for the Rust UI host's boot and mount — the layer that had none, and where the critical bug was.
//
// WHAT WENT WRONG, so this file is not just a green light: the mount logic lived inside a React effect that guarded
// itself with a ref plus a `disposed` flag. React 19 runs effects twice in development. The first run set the guard,
// StrictMode's cleanup set `disposed`, and the second run returned early — so the module was never mounted, no error was
// raised, and every screen using the host sat on an empty page forever. The previews were "verified" by looking at
// nothing.
//
// The lesson this file encodes: THE SECOND RUN MUST DO THE WORK. A test that runs the effect once cannot see the bug,
// so the regression test is the one below about running twice — and it is written against the plain module rather than
// a component, because that is the only way it can be written at all.
import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  bootRustUi,
  mountScreen,
  serveEffect,
  resetBoot,
  EFFECT_EVENT,
  ISLAND_EVENT,
  type RustUiModule,
} from '../lib/rust-ui/boot'

/** A stand-in for the wasm module, recording what was asked of it. */
function fakeModule(overrides: Partial<RustUiModule> = {}) {
  const calls = { init: 0, mount: 0, rowsLoaded: [] as string[] }
  const module: RustUiModule = {
    default: async () => {
      calls.init += 1
      return {}
    },
    mount: (_elementId, start) => {
      calls.mount += 1
      return JSON.stringify([{ effect: 'FetchRows', screen: start, scope: null }])
    },
    rows_loaded: (payload) => {
      calls.rowsLoaded.push(payload)
    },
    mount_id: () => 'rust-ui',
    effect_event_name: () => EFFECT_EVENT,
    island_event_name: () => ISLAND_EVENT,
    ...overrides,
  }
  return { module, calls }
}

const rowsResponse = (body: string) => ({
  ok: true,
  status: 200,
  text: async () => body,
})

test('the module is initialised once, however many callers ask', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  await bootRustUi(async () => module)
  await bootRustUi(async () => module)
  assert.equal(calls.init, 1, 'the wasm must not be initialised twice: the shell holds one program')
})

test('two callers arriving during the first load share it rather than both fetching', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  let loads = 0
  const load = async () => {
    loads += 1
    return module
  }
  const [first, second] = await Promise.all([bootRustUi(load), bootRustUi(load)])
  assert.equal(loads, 1, 'the promise is cached, not the result')
  assert.equal(first, second)
  assert.equal(calls.init, 1)
})

test('runs twice, and the second run still mounts — the bug that killed every page in development', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  const options = {
    rowsPath: '/api/portal/rust-ui/rows',
    start: 'activity',
    fetchRows: async () => rowsResponse('[{"id":"a","cells":["x"]}]'),
  }

  const booted = await bootRustUi(async () => module)
  // React 19 StrictMode: mount, cleanup, mount again. The old host aborted the second run and mounted nothing at all.
  await mountScreen(booted, options)
  await mountScreen(booted, options)

  assert.equal(calls.mount, 2, 'the second effect run must mount, not return early')
  assert.equal(calls.rowsLoaded.length, 2, 'and it must ask for the screen rows')
})

test('a detail screen is fetched with its record key', async () => {
  resetBoot()
  const { module } = fakeModule()
  const urls: string[] = []
  const booted = await bootRustUi(async () => module)
  const options = {
    rowsPath: '/api/portal/rust-ui/rows',
    start: 'client-record',
    scope: 'person-1',
    fetchRows: async (url: string) => {
      urls.push(url)
      return rowsResponse('[]')
    },
  }

  await serveEffect(booted, options, { effect: 'FetchRows', screen: 'client-record', scope: 'person-1' })
  assert.equal(urls[0], '/api/portal/rust-ui/rows?screen=client-record&scope=person-1')
})

test('a list screen does not inherit the record key it was navigated away from', async () => {
  resetBoot()
  const { module } = fakeModule()
  const urls: string[] = []
  const booted = await bootRustUi(async () => module)
  await serveEffect(
    booted,
    {
      rowsPath: '/api/portal/rust-ui/rows',
      start: 'client-record',
      scope: 'person-1',
      fetchRows: async (url: string) => {
        urls.push(url)
        return rowsResponse('[]')
      },
    },
    { effect: 'FetchRows', screen: 'clients', scope: null },
  )
  assert.ok(!urls[0].includes('scope'), `stale scope in ${urls[0]}`)
})

test('a refused fetch is an error, never an empty list', async () => {
  resetBoot()
  const { module } = fakeModule()
  const booted = await bootRustUi(async () => module)
  await assert.rejects(
    () =>
      serveEffect(
        booted,
        {
          rowsPath: '/api/portal/rust-ui/rows',
          start: 'activity',
          fetchRows: async () => ({ ok: false, status: 401, text: async () => '' }),
        },
        { effect: 'FetchRows', screen: 'activity', scope: null },
      ),
    /401/,
    '"nothing to show" and "we could not ask" are different states',
  )
})

test('an effect that is not a fetch is left alone', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  const booted = await bootRustUi(async () => module)
  await serveEffect(
    booted,
    { rowsPath: '/x', start: 'activity', fetchRows: async () => rowsResponse('[]') },
    { effect: 'SomethingElse' },
  )
  assert.deepEqual(calls.rowsLoaded, [])
})

test('an effect-name mismatch fails loudly instead of loading nothing', async () => {
  resetBoot()
  const { module } = fakeModule({ effect_event_name: () => 'rust-ui:wrong' })
  await assert.rejects(() => bootRustUi(async () => module), /effect event mismatch/)
})

test('an island-name mismatch warns and carries on', async () => {
  resetBoot()
  const { module } = fakeModule({ island_event_name: () => 'rust-ui:wrong' })
  const warnings: string[] = []
  const originalWarn = console.warn
  console.warn = (message: string) => warnings.push(message)
  try {
    await bootRustUi(async () => module)
  } finally {
    console.warn = originalWarn
  }
  assert.equal(warnings.length, 1, 'a warning, not a thrown error: islands must not take the screen down')
  assert.match(warnings[0], /island event mismatch/)
})

