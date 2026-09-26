// Tests for the Rust UI's boot — the one piece of the mount that can be tested without a DOM.
//
// WHAT WENT WRONG, so this file is not just a green light. The mount logic used to live inside a React effect that
// guarded itself with a ref plus a `disposed` flag. React 19 runs effects twice in development: the first run set the
// guard, StrictMode's cleanup set `disposed`, and the second run returned early — so the module was never mounted, no
// error was raised, and every page using the host sat on an empty container for the life of the page. The previews were
// "verified" by looking at nothing.
//
// THE LESSON, which is why the boot lives in a plain module and not in the component: **the second run must do the
// work**. A test that runs the effect once cannot see that bug, so the regression test below boots twice — and it is
// written against the plain module rather than a component, because that is the only way it can be written at all.
import { test } from 'node:test'
import assert from 'node:assert/strict'

import { bootRustUi, resetBoot, type RustUiModule } from '../lib/rust-ui/mount'

/** A stand-in for the wasm module, recording what was asked of it. */
function fakeModule() {
  const calls = { init: 0, loads: 0, startedIn: [] as Element[] }
  const module: RustUiModule = {
    default: async () => {
      calls.init += 1
      return {}
    },
    start_in: (root: Element) => {
      calls.startedIn.push(root)
    },
  }
  return { module, calls }
}

test('the module is loaded once, however many times the page asks', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  let loads = 0
  const load = async () => {
    loads += 1
    return module
  }

  await bootRustUi(load, '/rust-ui/ui_bg.wasm')
  await bootRustUi(load, '/rust-ui/ui_bg.wasm')
  await bootRustUi(load, '/rust-ui/ui_bg.wasm')

  assert.equal(loads, 1, 'the module was fetched more than once')
  assert.equal(calls.init, 1, 'the module was initialised more than once')
})

test('two callers arriving during the first load wait for the same one', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  let loads = 0
  const load = async () => {
    loads += 1
    return module
  }

  // Concurrent, which is the shape a double-invoked effect produces: the second call happens before the first resolves.
  const [first, second] = await Promise.all([
    bootRustUi(load, '/rust-ui/ui_bg.wasm'),
    bootRustUi(load, '/rust-ui/ui_bg.wasm'),
  ])

  assert.equal(loads, 1, 'the two callers each fetched the module')
  assert.equal(calls.init, 1, 'the two callers each initialised the module')
  assert.equal(first, second, 'the two callers got different modules')
})

test('resetBoot is what makes a page start over, and nothing else does', async () => {
  resetBoot()
  const { module, calls } = fakeModule()
  const load = async () => module

  await bootRustUi(load, '/rust-ui/ui_bg.wasm')
  resetBoot()
  await bootRustUi(load, '/rust-ui/ui_bg.wasm')

  assert.equal(calls.init, 2, 'resetBoot did not clear the cached boot')
})
