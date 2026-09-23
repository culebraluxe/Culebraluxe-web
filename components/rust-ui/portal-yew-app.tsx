'use client'

import { useEffect, useState } from 'react'

import { ProjectReactIslands } from '@/components/rust-ui/project-react-islands'
import { TechCockpitReactIslands } from '@/components/rust-ui/tech-cockpit-react-islands'
import { FlightRecorderReactIsland } from '@/components/rust-ui/flight-recorder-react-island'
import { UiLabReactIslands } from '@/components/rust-ui/ui-lab-react-islands'

// ---------------------------------------------------------------------------
// THE PORTAL APP'S MOUNT POINT.
//
// Same shape as the public one: boot the module and call `portal_mount` for the screen. Yew owns application state,
// effects and network. Projects adds one rendering-only React adapter for SVAR/FullCalendar vendor islands; those islands
// receive read-only props from Yew-owned DOM slots and never mutate application state.
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
      {screen === 'projects' ? <ProjectReactIslands /> : null}
      {screen === 'tech' ? <TechCockpitReactIslands /> : null}
      {screen === 'trace-record' ? <FlightRecorderReactIsland /> : null}
      {screen === 'design-lab' ? <UiLabReactIslands /> : null}
      {error ? (
        <p className="p-6 text-sm text-destructive" role="alert">
          The portal could not start: {error}
        </p>
      ) : null}
    </>
  )
}
