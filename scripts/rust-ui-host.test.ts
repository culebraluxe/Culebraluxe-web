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
  const calls = {
    init: 0,
    mount: 0,
    /** Every mount as `start@generation`, so a test can see which run asked. */
    mounts: [] as string[],
    /** Every rows payload delivered, as `screen@generation`, so a test can see whose answer landed. */
    rowsLoaded: [] as string[],
    pagesLoaded: [] as string[],
  }
  const module: RustUiModule = {
    default: async () => {
      calls.init += 1
      return {}
    },
    mount: (_elementId, start, generation) => {
      calls.mount += 1
      calls.mounts.push(`${start}@${generation}`)
      return JSON.stringify([{ effect: 'FetchRows', screen: start, scope: null, generation }])
    },
    rows_loaded: (screen, generation, payload) => {
      calls.rowsLoaded.push(`${screen}@${generation}:${payload}`)
    },
    page_loaded: (screen, generation, payload) => {
      calls.pagesLoaded.push(`${screen}@${generation}:${payload}`)
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



// ---------------------------------------------------------------------------
// RUN OWNERSHIP: what the browser showed when an old host run outlived its screen.
//
// The symptom was "every page renders Buyers": a run that had been replaced could still deliver, so a payload or a mount
// from the run the visitor had left could land on the run they were on. These tests pin the ordering, because the defect
// is an ORDERING bug and a test that cannot hold one request open cannot see it.
// ---------------------------------------------------------------------------

/** A fetch that does not resolve until the test says so — the only way to hold one request inside another. */
function deferred() {
  let release!: (value: string) => void
  const promise = new Promise<string>((resolve) => {
    release = resolve
  })
  return { promise, release }
}

test('a payload fetched by a superseded run is discarded, and the current run still gets its own', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  const booted = await bootRustUi(async () => module)
  const held = deferred()

  // Run 1 mounts Buyers and asks for its page. The request hangs.
  let current = 1
  const slow = serveEffect(
    booted,
    {
      rowsPath: '/api/rust-ui/public-rows',
      pagePath: '/api/rust-ui/public-page',
      start: 'site-buyers',
      generation: 1,
      isCurrent: () => current === 1,
      fetchRows: async () => ({ ok: true, status: 200, text: () => held.promise }),
    },
    { effect: 'FetchPage', screen: 'site-buyers', scope: null, generation: 1 },
  )

  // The visitor navigates: run 2 is current now, and its own request completes first.
  current = 2
  const held2 = deferred()
  const fast = serveEffect(
    booted,
    {
      rowsPath: '/api/rust-ui/public-rows',
      pagePath: '/api/rust-ui/public-page',
      start: 'site-home',
      generation: 2,
      isCurrent: () => current === 2,
      fetchRows: async () => ({ ok: true, status: 200, text: () => held2.promise }),
    },
    { effect: 'FetchPage', screen: 'site-home', scope: null, generation: 2 },
  )
  held2.release('{"hero":{"title":"Home"}}')
  await fast

  // Now the old request completes. It must not reach Rust.
  held.release('{"hero":{"title":"Buyers"}}')
  await slow

  assert.deepEqual(
    calls.pagesLoaded,
    ['site-home@2:{"hero":{"title":"Home"}}'],
    'the superseded run’s page must be dropped and only the current run’s delivered',
  )
})

test('an effect event from a previous program is not this run’s to serve', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  const booted = await bootRustUi(async () => module)
  const current: number = 2

  // The current run holds its own request open while an effect from the generation before it arrives late.
  const held = deferred()
  const newer = serveEffect(
    booted,
    {
      rowsPath: '/api/rust-ui/public-rows',
      pagePath: '/api/rust-ui/public-page',
      start: 'site-home',
      generation: 2,
      isCurrent: () => current === 2,
      fetchRows: async () => ({ ok: true, status: 200, text: () => held.promise }),
    },
    { effect: 'FetchPage', screen: 'site-home', scope: null, generation: 2 },
  )
  await serveEffect(
    booted,
    {
      rowsPath: '/api/rust-ui/public-rows',
      pagePath: '/api/rust-ui/public-page',
      start: 'site-home',
      generation: 2,
      isCurrent: () => current === 2,
      fetchRows: async () => ({ ok: true, status: 200, text: () => Promise.resolve('[]') }),
    },
    { effect: 'FetchPage', screen: 'site-buyers', scope: null, generation: 1 },
  )

  held.release('{"hero":{"title":"Home"}}')
  await newer

  assert.deepEqual(
    calls.pagesLoaded,
    ['site-home@2:{"hero":{"title":"Home"}}'],
    'the stale generation’s effect was never fetched, so it delivered nothing',
  )
})

test('an obsolete run may not mount — the old screen cannot be re-opened over the current one', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  const booted = await bootRustUi(async () => module)
  const current: number = 2

  const effects = await mountScreen(booted, {
    rowsPath: '/api/rust-ui/public-rows',
    start: 'site-buyers',
    generation: 1,
    isCurrent: () => current === 1,
    fetchRows: async () => rowsResponse('[]'),
  })

  assert.deepEqual(effects, [])
  assert.equal(calls.mount, 0, 'an obsolete run must not reach `mount` at all')
  assert.deepEqual(calls.rowsLoaded, [], 'and it must not deliver anything')
})

test('a run replaced while its first effect is in flight does not serve the rest of them', async () => {
  resetBoot()
  const { module, calls } = fakeModule({
    mount: (_elementId, start, generation) =>
      JSON.stringify([
        { effect: 'FetchRows', screen: start, scope: null, generation },
        { effect: 'FetchRows', screen: start, scope: null, generation },
      ]),
  })
  const booted = await bootRustUi(async () => module)
  const held = deferred()
  let current = 1

  const mounted = mountScreen(booted, {
    rowsPath: '/api/rust-ui/public-rows',
    start: 'site-buyers',
    generation: 1,
    isCurrent: () => current === 1,
    fetchRows: async () => ({ ok: true, status: 200, text: () => held.promise }),
  })
  // The visitor leaves while the first request is in flight.
  current = 2
  held.release('[]')
  await mounted

  assert.equal(
    calls.rowsLoaded.length,
    0,
    'nothing at all from an obsolete run: the effect already in flight is dropped at the boundary before Rust',
  )
})

test('both runs mount while they are current, which is the StrictMode case that must still work', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  const booted = await bootRustUi(async () => module)
  let current = 0
  const run = async () => {
    const generation = ++current
    return mountScreen(booted, {
      rowsPath: '/api/rust-ui/public-rows',
      start: 'activity',
      generation,
      isCurrent: () => current === generation,
      fetchRows: async () => rowsResponse('[]'),
    })
  }

  // React 19: setup, cleanup, setup. The SECOND run must do the work — the old host made it give up and left every page
  // empty — and BOTH must mount, each under its own generation.
  await run()
  await run()

  assert.deepEqual(calls.mounts, ['activity@1', 'activity@2'], 'each run mounts, and says which run it is')
  assert.equal(calls.rowsLoaded.length, 2, 'and each run is served its own answer')
})

