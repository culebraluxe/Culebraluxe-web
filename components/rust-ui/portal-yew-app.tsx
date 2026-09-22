'use client'

import { useEffect, useState } from 'react'

// ---------------------------------------------------------------------------
// THE PORTAL APP'S MOUNT POINT.
//
// Same shape as the public one and the same smallness: boot the module, call `portal_mount` with the screen the page
// serves, and stop. No state, no effect, no listener, no fetch — the Yew application owns all of it from there.
// ---------------------------------------------------------------------------

type RustUiModule = {
  default: (init?: { module_or_path?: string }) => Promise<unknown>
  portal_mount: (elementId: string, screenKey: string, scope: string) => void
}

/** Booted once per document, and mounted for the one screen this page serves. */
let booted: Promise<RustUiModule> | null = null

async function boot(wasmPath: string): Promise<RustUiModule> {
  booted ??= (async () => {
    const module = (await import('@/lib/rust-ui/ui.js')) as unknown as RustUiModule
    await module.default({ module_or_path: wasmPath })
    return module
  })()
  return booted
}

export function PortalYewApp({
  screen,
  scope,
  wasmPath = '/rust-ui/ui_bg.wasm',
}: {
  screen: string
  scope?: string
  wasmPath?: string
}) {
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    void boot(wasmPath)
      .then((module) => {
        if (!cancelled) module.portal_mount('rust-ui', screen, scope ?? '')
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause))
      })
    return () => {
      cancelled = true
    }
  }, [screen, scope, wasmPath])

  return (
    <>
      <div id="rust-ui" />
      {error ? (
        <p className="p-6 text-sm text-destructive" role="alert">
          The portal could not start: {error}
        </p>
      ) : null}
    </>
  )
}
