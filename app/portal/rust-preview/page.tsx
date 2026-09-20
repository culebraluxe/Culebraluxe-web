'use client'

import { useEffect, useRef, useState } from 'react'

// ---------------------------------------------------------------------------
// THE RUST UI HOST.
//
// This page is the whole TypeScript side of the port, and it is deliberately thin: it mounts the WASM module into a
// container, listens for the effects the Rust side announces, fetches what they ask for from this application's own
// routes, and hands the JSON back. It renders no screen furniture of its own — every heading, list and button below
// `#rust-ui` comes from `rust/ui/src/view.rs`.
//
// WHY THE HOST OWNS THE NETWORK: the session cookie, the internal key and the permission checks live here. The WASM
// module holds no credential and performs no request, so the worst a bug in the view layer can do is ask for data it
// is already allowed to see.
//
// This page is scaffolding for the port, not the destination: as each screen is finished, its real `/portal/*` route
// becomes the host for that screen and this preview stays behind as the place to exercise the shell alone.
// ---------------------------------------------------------------------------

/** The effect names `rust/ui/src/model.rs` can ask for. Kept in sync by hand, and checked below. */
const FETCH_ROWS = 'FetchRows'

/** The DOM event `rust/ui/src/shell.rs` announces effects on (`EFFECT_EVENT`). */
const EFFECT_EVENT = 'rust-ui:effects'

type RustUi = {
  default: (init?: { module_or_path?: string }) => Promise<unknown>
  mount: (elementId: string) => string
  rows_loaded: (payload: string) => void
  mount_id: () => string
  effect_event_name: () => string
}

export default function RustPreviewPage() {
  const started = useRef(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    // React 19 runs effects twice in development. Without this guard the module mounts twice and two click listeners
    // race on the same DOM subtree.
    if (started.current) return
    started.current = true

    let disposed = false

    const loadRows = async (module: RustUi, screen: string) => {
      const response = await fetch(`/api/portal/rust-ui/rows?screen=${encodeURIComponent(screen)}`)
      if (!response.ok) {
        // A refused fetch must become a visible message, not a silent empty list: "nothing to show" and "we could
        // not ask" are different states and the user deserves the difference.
        throw new Error(`rows request failed with ${response.status}`)
      }
      module.rows_loaded(await response.text())
    }

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

      // The shell paints the new screen and *then* announces the effects, so the screen to fetch is simply the one
      // now on the page. One source of truth for "where are we": the model that just rendered it.
      const currentScreen = () =>
        document.querySelector('[data-rust-screen]')?.getAttribute('data-rust-screen') ?? null

      document.addEventListener(EFFECT_EVENT, (event) => {
        const detail = (event as CustomEvent<string>).detail
        const effects = JSON.parse(detail) as { effect: string }[]
        const screen = currentScreen()
        if (!screen || !effects.some((effect) => effect.effect === FETCH_ROWS)) return
        void loadRows(module, screen).catch((cause: unknown) => {
          setError(cause instanceof Error ? cause.message : String(cause))
        })
      })

      // The effects caused by opening the first screen come back from `mount`, so the first load does not wait for a
      // click — the page arrives with data instead of a list that fills in once you touch something.
      const initial = JSON.parse(module.mount(module.mount_id())) as { effect: string }[]
      const first = currentScreen()
      if (first && initial.some((effect) => effect.effect === FETCH_ROWS)) {
        await loadRows(module, first)
      }
    }

    const cleanup = run().catch((cause: unknown) => {
      setError(cause instanceof Error ? cause.message : String(cause))
    })

    return () => {
      disposed = true
      void cleanup
    }
  }, [])

  return (
    <div className="min-h-screen bg-background">
      <div id="rust-ui" />
      {error ? (
        <p className="border-t border-destructive/40 bg-destructive/10 p-3 text-sm" role="alert">
          Rust UI host error: {error}
        </p>
      ) : null}
    </div>
  )
}
