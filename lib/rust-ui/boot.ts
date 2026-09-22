// The Rust UI's boot and mount, as plain TypeScript.
//
// WHY THIS FILE EXISTS, AND WHY IT HAD NO TEST BEFORE. This is where the critical bug was: the React effect guarded
// itself with a ref plus a `disposed` flag. React 19 runs effects twice in development — the first run set the guard,
// StrictMode's cleanup set `disposed`, and the second run returned early — so the module was never mounted and the page
// stayed empty FOREVER, with no error and nothing in the console. Every screen using the host was dead in development,
// previews included.
//
// Nothing caught it because nothing tested this path: it lived inside a React component, where a test needs a DOM. So
// the logic lives here instead — no React, no DOM, no browser — and a test can call it twice and assert what happens.

/** The module wasm-bindgen generates. Only the parts the host uses. */
export type RustUiModule = {
  default: (init?: { module_or_path?: string }) => Promise<unknown>
  mount: (elementId: string, start: string) => string
  rows_loaded: (payload: string) => void
  /** Present so a page payload can land on a build that predates it without taking the screen down. */
  page_loaded?: (payload: string) => void
  mount_id: () => string
  effect_event_name: () => string
  island_event_name?: () => string
}

/** The effects `rust/ui/src/model.rs` can ask for. */
const FETCH_ROWS = 'FetchRows'
const FETCH_PAGE = 'FetchPage'

/** The DOM events `rust/ui/src/shell.rs` announces on. */
export const EFFECT_EVENT = 'rust-ui:effects'
export const ISLAND_EVENT = 'rust-ui:islands'

export type RustUiEffect = { effect: string; screen?: string; scope?: string | null }

let booted: Promise<RustUiModule> | null = null

/**
 * Load and initialise the module once per page.
 *
 * ONCE, because the module and the shell's program are single-instance by design: `rust/ui/src/shell.rs` keeps one
 * program in a thread-local, so booting twice would fight over one program. The PROMISE is cached rather than the
 * result, so two callers arriving during the first load wait for the same one instead of both fetching.
 */
export async function bootRustUi(
  load: () => Promise<RustUiModule>,
  wasmPath = '/rust-ui/ui_bg.wasm',
): Promise<RustUiModule> {
  booted ??= (async () => {
    const module = await load()
    await module.default({ module_or_path: wasmPath })
    // Asked for rather than assumed: a mismatch would present as a screen that quietly never loads.
    const effectEvent = module.effect_event_name()
    if (effectEvent !== EFFECT_EVENT) {
      throw new Error(`effect event mismatch: Rust says "${effectEvent}", host expects "${EFFECT_EVENT}"`)
    }
    const islandEvent = module.island_event_name?.()
    if (islandEvent !== ISLAND_EVENT) {
      // A warning rather than a failure: islands not mounting must not take the screen down with them.
      console.warn(
        `Rust UI island event mismatch: Rust says "${islandEvent}", host expects "${ISLAND_EVENT}". Islands will not mount.`,
      )
    }
    return module
  })()
  return booted
}


export type RowsResponse = { ok: boolean; status: number; text: () => Promise<string> }

export type MountOptions = {
  rowsPath: string
  /**
   * Where a page's blocks come from, for the screens that are pages rather than lists.
   *
   * Optional because it is a public-site concern today: the portal has no page route, and a screen that asks for a page
   * where none is configured is told so rather than fetching a path that does not exist.
   */
  pagePath?: string
  start: string
  /** The record key this page is about, when it is a detail route. */
  scope?: string
  /** Perform the rows request. The host owns the network, so this is injected: the browser passes `fetch`. */
  fetchRows: (url: string) => Promise<RowsResponse>
  /** Called with the parsed rows, before Rust is told about them. */
  onRows?: (rows: unknown) => void
}

/**
 * Serve one effect.
 *
 * Extracted so the effects a CLICK produces go through the same code as the effects the first mount produces — the old
 * host had this logic inline in an effect, so the two paths could drift apart and only one of them was reachable.
 */
export async function serveEffect(
  module: RustUiModule,
  options: Pick<MountOptions, 'rowsPath' | 'pagePath' | 'start' | 'scope' | 'fetchRows' | 'onRows'>,
  effect: RustUiEffect,
): Promise<void> {
  const screen = effect.screen
  if (!screen) return

  if (effect.effect === FETCH_PAGE) {
    if (!options.pagePath) {
      throw new Error(`screen "${screen}" asked for its page but no page feed is configured for this host`)
    }
    // A page can be about one record, exactly as a row list can. The detail route has no row to click — the slug comes
    // from the URL, which the page hands in — so it travels the same way the rows route's scope does: the host knows the
    // URL, Rust knows what it is showing, and the key is exchanged rather than scraped out of the DOM.
    const query = new URLSearchParams({ screen })
    const key = effect.scope ?? (screen === options.start ? options.scope : null)
    if (key) query.set('scope', key)
    const response = await options.fetchRows(`${options.pagePath}?${query.toString()}`)
    if (!response.ok) throw new Error(`page request failed with ${response.status}`)
    module.page_loaded?.(await response.text())
    return
  }

  if (effect.effect !== FETCH_ROWS) return
  const query = new URLSearchParams({ screen })
  // A detail screen is about one record; the key travels with the effect rather than being scraped out of the DOM.
  const key = effect.scope ?? (screen === options.start ? options.scope : null)
  if (key) query.set('scope', key)
  const response = await options.fetchRows(`${options.rowsPath}?${query.toString()}`)
  if (!response.ok) {
    // A refused fetch must become a visible message, not a silent empty list: "nothing to show" and "we could not ask"
    // are different states and the user deserves the difference.
    throw new Error(`rows request failed with ${response.status}`)
  }
  const payload = await response.text()
  if (options.onRows) {
    try {
      options.onRows(JSON.parse(payload))
    } catch {
      options.onRows([])
    }
  }
  module.rows_loaded(payload)
}

/**
 * Mount `start` into the module's container and serve every effect it asks for.
 *
 * SAFE TO CALL ON EVERY EFFECT RUN, which is the whole point: the old code tried to PREVENT the second run instead of
 * being idempotent, and that prevention is what killed the page. Mounting twice is harmless — the shell replaces the
 * program and repaints the same container.
 */
export async function mountScreen(module: RustUiModule, options: MountOptions): Promise<RustUiEffect[]> {
  const effects = JSON.parse(module.mount(module.mount_id(), options.start)) as RustUiEffect[]
  for (const effect of effects) {
    await serveEffect(module, options, effect)
  }
  return effects
}

/** Forget the cached boot. For tests, and for a page that deliberately starts over. */
export function resetBoot(): void {
  booted = null
}
