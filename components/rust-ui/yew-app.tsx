'use client'

import { useEffect, useState } from 'react'

// ---------------------------------------------------------------------------
// THE YEW APP'S MOUNT POINT — and the whole of what the browser does.
//
// NO LIFECYCLE, NO EFFECTS, NO FETCHES, NO LISTENERS. It boots the module and calls `yew_mount`; from there the Yew
// router owns the URL, the reducer owns the state, the components own the DOM, and the effects are async functions in
// Rust. The previous host existed because the string renderer could not do any of that itself; this one has nothing to
// do but start the app.
//
// WHY THE MODULE-SCOPE PROMISE: `started ??=` makes the boot idempotent, so React 19's double-invoked effect in
// development cannot mount two applications into one container. It is a guard around MOUNTING — not around the second
// run doing its work, which is the distinction that broke the old host.
// ---------------------------------------------------------------------------

type RustUiModule = {
  default: (init?: { module_or_path?: string }) => Promise<unknown>
  yew_mount: (elementId: string) => void
}

let started: Promise<void> | null = null

function startYew(wasmPath: string): Promise<void> {
  started ??= (async () => {
    const module = (await import('@/lib/rust-ui/ui.js')) as unknown as RustUiModule
    await module.default({ module_or_path: wasmPath })
    module.yew_mount('rust-ui')
  })()
  return started
}

export function YewApp({ wasmPath = '/rust-ui/ui_bg.wasm' }: { wasmPath?: string }) {
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void startYew(wasmPath).catch((cause: unknown) =>
      setError(cause instanceof Error ? cause.message : String(cause)),
    )
  }, [wasmPath])

  return (
    <>
      <div id="rust-ui" />
      {error ? (
        <p className="p-6 text-sm text-destructive" role="alert">
          The application could not start: {error}
        </p>
      ) : null}
    </>
  )
}
