// The Rust UI's boot, as plain TypeScript with no React and no DOM.
//
// WHY THIS IS A MODULE AND NOT A REACT EFFECT'S PRIVATE BUSINESS. The mount logic used to live inside a React effect
// that guarded itself with a ref plus a `disposed` flag. React 19 runs effects twice in development: the first run set
// the guard, StrictMode's cleanup set `disposed`, and the second run returned early — so the module was never mounted,
// no error was raised, and every page sat on an empty container forever. The previews were "verified" by looking at
// nothing.
//
// THE RULE, kept here where a test can hold it without a browser: **the second run must do the work**, and booting the
// module twice must be idempotent rather than prevented. The component that calls this invalidates its own obsolete
// runs; this module only ever promises one boot.

/** The module wasm-bindgen generates. Only the parts the mount host uses. */
export type RustUiModule = {
  default: (init?: { module_or_path?: string }) => Promise<unknown>
  /**
   * Start the application in the container the page owns, handed over by reference.
   *
   * THE ELEMENT IS PASSED, NOT LOOKED UP. `#rust-ui` is an id, and an id is not an identity: during a client-side
   * navigation Next renders the new route while the old one is still in the document, so two containers can carry that
   * id at once and a lookup answers with the first — the page being left. See `shell::start_in`.
   */
  start_in: (root: Element) => void
}

let booted: Promise<RustUiModule> | null = null

/**
 * Load and initialise the module once per document.
 *
 * ONCE, because the module keeps one application per container in a thread-local and a second wasm instance would be a
 * second application with its own copy of the state. The PROMISE is cached rather than the result, so two callers
 * arriving during the first load wait for the same one instead of both fetching.
 */
export async function bootRustUi(
  load: () => Promise<RustUiModule>,
  wasmPath = '/rust-ui/ui_bg.wasm',
): Promise<RustUiModule> {
  booted ??= (async () => {
    const module = await load()
    await module.default({ module_or_path: wasmPath })
    return module
  })()
  return booted
}

/** Forget the cached boot. For tests, and for a page that deliberately starts over. */
export function resetBoot(): void {
  booted = null
}
