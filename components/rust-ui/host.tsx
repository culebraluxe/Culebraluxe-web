'use client'

import { useEffect, useRef, useState, type ReactNode } from 'react'
import { createRoot } from 'react-dom/client'

// ---------------------------------------------------------------------------
// THE RUST UI HOST.
//
// This component is the whole TypeScript side of the port, and it is deliberately thin: it mounts the WASM module into
// a container, listens for the effects the Rust side announces, fetches what they ask for from an application route,
// and hands the JSON back. It renders no screen furniture of its own — every heading, list, badge and button below
// `#rust-ui` comes from `rust/ui/src/view.rs`.
//
// WHY THE HOST OWNS THE NETWORK: the session cookie, the internal key and the permission checks live in TypeScript.
// The WASM module holds no credential and performs no request, so the worst a bug in the view layer can do is ask for
// data it is already allowed to see.
//
// WHY IT TAKES `rowsPath` AND `start`: the public site and the portal are two hosts of one shell. Neither the starting
// screen nor the route that serves rows is baked into the module, because the URL belongs to the page — which is also
// what keeps the public host from being able to ask the portal's endpoints for anything.
// ---------------------------------------------------------------------------

/** The effect names `rust/ui/src/model.rs` can ask for. Kept in sync by hand, and checked below. */
const FETCH_ROWS = 'FetchRows'

/** The DOM event `rust/ui/src/shell.rs` announces effects on (`EFFECT_EVENT`). */
const EFFECT_EVENT = 'rust-ui:effects'

/** The DOM event the shell announces islands on. Checked against Rust the same way the effect name is. */
const ISLAND_EVENT = 'rust-ui:islands'

type RustUiEffect = { effect: string; screen?: string; scope?: string | null }

type RustUi = {
  default: (init?: { module_or_path?: string }) => Promise<unknown>
  mount: (elementId: string, start: string) => string
  rows_loaded: (payload: string) => void
  mount_id: () => string
  effect_event_name: () => string
  island_event_name: () => string
}

/**
 * A third-party widget Rust renders a container for.
 *
 * Rust owns the box (`<div data-island="projects-tree">`); the widget owns everything inside it. The widget is handed
 * the rows the same fetch the screen used returned, because the host is what owns the network and the widget must not
 * grow a second one.
 */
export type RustIsland = (props: { rows: RustUiRow[] }) => ReactNode

export type RustUiRow = { id: string; cells: string[]; badge?: string | null }

export function RustUiHost({
  rowsPath,
  start,
  scope,
  islands,
}: {
  rowsPath: string
  start: string
  /**
   * The record key this page is about, for a detail route (`/portal/clients/[personId]`). A screen reached by clicking a
   * row gets its key from the effect Rust emits; a screen reached by typing the URL has no row to be clicked, so the
   * page hands the key in. Applied to the FIRST screen only - navigating on to a list must not inherit one record's key.
   */
  scope?: string
  /**
   * Third-party widgets Rust renders containers for, by island name (`projects-tree`). Rust paints the box; these own
   * what is inside it. Nothing is mounted into a container that already has a child, so a widget that survived a paint
   * is left alone rather than remounted under the user's cursor.
   */
  islands?: Record<string, RustIsland>
}) {
  const started = useRef(false)
  const [error, setError] = useState<string | null>(null)
  // Refs rather than state: the island event fires from the WASM side, outside a React render, so it must read the
  // current rows and the current registry without being re-registered on every render.
  const rowsRef = useRef<RustUiRow[]>([])
  const islandsRef = useRef(islands)
  const rootsRef = useRef(new Map<string, ReturnType<typeof createRoot>>())
  islandsRef.current = islands

  useEffect(() => {
    // React 19 runs effects twice in development. Without this guard the module mounts twice and two click listeners
    // race on the same DOM subtree.
    if (started.current) return
    started.current = true

    let disposed = false

    const run = async () => {
      const module = (await import('@/lib/rust-ui/ui.js')) as unknown as RustUi
      if (disposed) return
      await module.default({ module_or_path: '/rust-ui/ui_bg.wasm' })
      if (disposed) return

      // The event name is asked for rather than assumed: it is declared in Rust, and a mismatch would present as a
      // screen that quietly never loads.
      const eventName = module.effect_event_name()
      if (eventName !== EFFECT_EVENT) {
        throw new Error(`effect event mismatch: Rust says "${eventName}", host expects "${EFFECT_EVENT}"`)
      }
      // Same check for the island event, but this one must not be fatal: a mismatch would present as a widget that
      // never appears, and refusing to render the whole screen over it would be the worse failure. It is a warning and
      // the screen carries on.
      const islandEvent = module.island_event_name?.()
      if (islandEvent !== ISLAND_EVENT) {
        console.warn(
          `Rust UI island event mismatch: Rust says "${islandEvent}", host expects "${ISLAND_EVENT}". Islands will not mount.`,
        )
      }

      /**
       * Put each registered widget into the container Rust rendered for it.
       *
       * A container that already has a child keeps it: Rust re-paints its markup on every message, and a widget that
       * survived that paint must not be torn down and rebuilt under the user's cursor.
       */
      const mountIslands = () => {
        const registry = islandsRef.current
        if (!registry) return
        for (const [name, Widget] of Object.entries(registry)) {
          const container = document.querySelector(`[data-island="${name}"]`)
          if (!container) continue
          if (container.childElementCount > 0) continue
          // The container is a new node after every paint, so the previous root points at a detached one.
          rootsRef.current.get(name)?.unmount()
          const root = createRoot(container)
          rootsRef.current.set(name, root)
          root.render(<Widget rows={rowsRef.current} />)
        }
      }

      document.addEventListener(ISLAND_EVENT, mountIslands)

      const loadRows = async (effect: RustUiEffect) => {
        const screen = effect.screen
        if (!screen) return
        const query = new URLSearchParams({ screen })
        // A detail screen is about one record; the key travels with the effect rather than being scraped back out of
        // the DOM. On the opening screen the page's own scope stands in when the effect carries none.
        const key = effect.scope ?? (screen === start ? scope : null)
        if (key) query.set('scope', key)
        const response = await fetch(`${rowsPath}?${query.toString()}`)
        if (!response.ok) {
          // A refused fetch must become a visible message, not a silent empty list: "nothing to show" and "we could
          // not ask" are different states and the user deserves the difference.
          throw new Error(`rows request failed with ${response.status}`)
        }
        const payload = await response.text()
        // Kept as well as handed over: an island widget is a React component and needs the rows as values.
        try {
          rowsRef.current = JSON.parse(payload) as RustUiRow[]
        } catch {
          rowsRef.current = []
        }
        module.rows_loaded(payload)
      }

      const handle = (effects: RustUiEffect[]) => {
        for (const effect of effects) {
          if (effect.effect !== FETCH_ROWS) continue
          void loadRows(effect).catch((cause: unknown) => {
            setError(cause instanceof Error ? cause.message : String(cause))
          })
        }
      }

      // Clicks happen inside the module's own listener, so their effects are announced on the document rather than
      // returned to JavaScript — the value returned by `mount` was consumed long ago.
      document.addEventListener(EFFECT_EVENT, (event) => {
        handle(JSON.parse((event as CustomEvent<string>).detail) as RustUiEffect[])
      })

      // `mount` returns the effects caused by opening the first screen, so the page arrives with data instead of a
      // list that fills in once you click something.
      handle(JSON.parse(module.mount(module.mount_id(), start)) as RustUiEffect[])
    }

    void run().catch((cause: unknown) => {
      setError(cause instanceof Error ? cause.message : String(cause))
    })

    return () => {
      disposed = true
    }
  }, [rowsPath, start, scope])

  return (
    <>
      <div id="rust-ui" />
      {error ? (
        <p className="border-t border-destructive/40 bg-destructive/10 p-3 text-sm" role="alert">
          Rust UI host error: {error}
        </p>
      ) : null}
    </>
  )
}
