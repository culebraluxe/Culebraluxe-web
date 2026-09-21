'use client'

import { useEffect, useRef, useState } from 'react'

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

type RustUiEffect = { effect: string; screen?: string; scope?: string | null }

type RustUi = {
  default: (init?: { module_or_path?: string }) => Promise<unknown>
  mount: (elementId: string, start: string) => string
  rows_loaded: (payload: string) => void
  mount_id: () => string
  effect_event_name: () => string
}

export function RustUiHost({ rowsPath, start }: { rowsPath: string; start: string }) {
  const started = useRef(false)
  const [error, setError] = useState<string | null>(null)

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

      const loadRows = async (effect: RustUiEffect) => {
        const screen = effect.screen
        if (!screen) return
        const query = new URLSearchParams({ screen })
        // A detail screen is about one record; the key travels with the effect rather than being scraped back out of
        // the DOM.
        if (effect.scope) query.set('scope', effect.scope)
        const response = await fetch(`${rowsPath}?${query.toString()}`)
        if (!response.ok) {
          // A refused fetch must become a visible message, not a silent empty list: "nothing to show" and "we could
          // not ask" are different states and the user deserves the difference.
          throw new Error(`rows request failed with ${response.status}`)
        }
        module.rows_loaded(await response.text())
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
  }, [rowsPath, start])

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
