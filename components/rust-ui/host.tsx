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

/**
 * The protocol and the mount logic live in `lib/rust-ui/boot.ts`, deliberately: they are the part that was wrong, and
 * this file cannot be tested without a DOM. What stays here is the React and DOM half — the wasm's container, the
 * widgets, and reading the events Rust announces.
 */
import {
  bootRustUi,
  mountScreen,
  serveEffect,
  EFFECT_EVENT,
  ISLAND_EVENT,
  type RustUiEffect,
  type RustUiModule,
} from '@/lib/rust-ui/boot'

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
  pagePath,
  start,
  scope,
  islands,
}: {
  rowsPath: string
  /**
   * Where a page's blocks come from, for screens that are pages rather than lists (the public site). Left out, a screen
   * that asks for its page says so loudly instead of fetching a path that does not exist.
   */
  pagePath?: string
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
  const [error, setError] = useState<string | null>(null)
  /**
   * Whether the module has mounted.
   *
   * THIS EXISTS BECAUSE A BLANK PAGE SAYS NOTHING. The container is empty until Rust paints into it, so the difference
   * between "the wasm is loading", "the wasm failed" and "React never ran at all" was invisible — all three looked
   * like a white page. The placeholder is a SIBLING of the mount point rather than its content, because React must not
   * own anything inside `#rust-ui`: Rust replaces that element's children and a React child there would collide.
   */
  const [mounted, setMounted] = useState(false)
  // Refs rather than state: the island event fires from the WASM side, outside a React render, so it must read the
  // current rows and the current registry without being re-registered on every render.
  const rowsRef = useRef<RustUiRow[]>([])
  const islandsRef = useRef(islands)
  const rootsRef = useRef(new Map<string, ReturnType<typeof createRoot>>())
  islandsRef.current = islands

  useEffect(() => {
    /**
     * The module is booted once and this effect mounts it on EVERY run.
     *
     * THE BUG THIS REPLACES: an earlier version guarded with a ref and a `disposed` flag, so React 19's second
     * development run returned early while the first had already been abandoned — leaving every page on an empty
     * container forever, with no error anywhere. `lib/rust-ui/boot.ts` holds the logic now, with tests that run the
     * mount twice, because this file cannot be tested without a DOM and that is exactly how the bug survived.
     */
    const fetchRows = async (url: string) => {
      const response = await fetch(url)
      // Logged because the alternative is guessing: the model sat on `loading: true` while the route answered 200, and
      // nothing on either side said which step was lost. One line per request settles it.
      console.info(`[rust-ui] rows ${url} -> ${response.status}`)
      return { ok: response.ok, status: response.status, text: () => response.text() }
    }
    const options = {
      rowsPath,
      pagePath,
      start,
      scope,
      fetchRows,
      onRows: (rows: unknown) => {
        rowsRef.current = Array.isArray(rows) ? (rows as RustUiRow[]) : []
      },
    }

    let module: RustUiModule | null = null

    /**
     * Put each registered widget into the container Rust rendered for it.
     *
     * A container that already has a child keeps it: Rust re-paints its markup on every message, so a widget that
     * survived a paint must not be torn down and rebuilt under the user's cursor.
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

    // Clicks happen inside the module's own listener, so their effects are announced on the document rather than
    // returned to JavaScript — the value returned by `mount` was consumed long ago.
    const onEffects = (event: Event) => {
      if (!module) return
      const effects = JSON.parse((event as CustomEvent<string>).detail) as RustUiEffect[]
      console.info(
        `[rust-ui] effects from the page: ${effects.map((e) => `${e.effect}:${e.screen ?? '-'}`).join(', ') || '(none)'}`,
      )
      for (const effect of effects) {
        void serveEffect(module, options, effect).catch((cause: unknown) =>
          setError(cause instanceof Error ? cause.message : String(cause)),
        )
      }
    }

    const run = async () => {
      module = await bootRustUi(() => import('@/lib/rust-ui/ui.js') as unknown as Promise<RustUiModule>)
      document.addEventListener(EFFECT_EVENT, onEffects)
      document.addEventListener(ISLAND_EVENT, mountIslands)
      // `mount` returns the effects caused by opening the first screen, so the page arrives with data instead of a list
      // that fills in once you click something.
      const effects = await mountScreen(module, options)
      mountIslands()
      setMounted(true)
      console.info(
        `[rust-ui] mounted "${start}" via ${rowsPath} - ${effects.length} effect(s); rows loaded: ${rowsRef.current.length}`,
      )
    }

    void run().catch((cause: unknown) => {
      setError(cause instanceof Error ? cause.message : String(cause))
    })

    return () => {
      // Removing this run's listeners is the whole cleanup. Note what is NOT here: no flag that makes the next run
      // give up. Aborting the work instead of being idempotent is what broke every page in development.
      document.removeEventListener(EFFECT_EVENT, onEffects)
      document.removeEventListener(ISLAND_EVENT, mountIslands)
    }
  }, [rowsPath, pagePath, start, scope])

  return (
    <>
      {!mounted && !error ? (
        <p className="p-6 text-sm text-muted-foreground" data-rust-ui-status="loading">
          Loading the Rust UI…
        </p>
      ) : null}
      <div id="rust-ui" />
      {error ? (
        <p className="border-t border-destructive/40 bg-destructive/10 p-3 text-sm" role="alert">
          Rust UI host error: {error}
        </p>
      ) : null}
    </>
  )
}
