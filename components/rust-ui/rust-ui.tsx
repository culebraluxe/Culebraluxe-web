'use client'

import { useEffect, useRef, useState } from 'react'

import { FlightRecorderReactIsland } from '@/components/rust-ui/flight-recorder-react-island'
import { OpsVideoReactIsland } from '@/components/rust-ui/opps-video-island'
import { ProjectReactIslands } from '@/components/rust-ui/project-react-islands'
import { TechCockpitReactIslands } from '@/components/rust-ui/tech-cockpit-react-islands'
import { UiLabReactIslands } from '@/components/rust-ui/ui-lab-react-islands'
import { bootRustUi, type RustUiModule } from '@/lib/rust-ui/mount'

// ---------------------------------------------------------------------------
// THE ONLY MOUNT POINT THE APPLICATION HAS.
//
// One component for both applications, because a page has exactly one owner and there is no reason for three spellings
// of "boot the module and hand it the node". This file decides almost nothing: it names the container, hands it over,
// and mounts the third-party widgets for the handful of screens where Rust renders only the box. Rust reads the
// attributes off the element and mounts the application itself — see `shell::start_in` and `shell::start_at`.
//
// A PAGE THAT NAMES NO SCREEN IS THE PUBLIC SITE. The site app owns its own URLs through the browser router, so a
// public page passes nothing at all; a portal page names its screen, because Next owns the `/portal/*` URL and Rust
// does not.
//
// THE ELEMENT IS HANDED OVER RATHER THAN LOOKED UP, and the ref is why: with a lookup by id, a page left in the
// document by a client-side navigation could answer for the page arriving, and the screen would mount into a container
// nobody could see. That was one of the three owners the portal used to have.
// ---------------------------------------------------------------------------

export function RustUi({
  screen,
  scope,
  error,
  wasmPath = '/rust-ui/ui_bg.wasm',
}: {
  /** The portal screen this page serves. Omitted on a public page, where the URL decides. */
  screen?: string
  /** The record key a detail route carries (`/portal/clients/[personId]`). */
  scope?: string
  /** The error boundary's page: neither a route nor a screen, rendered at whatever URL failed. */
  error?: boolean
  wasmPath?: string
}) {
  /**
   * WHICH APPLICATION THIS PAGE IS.
   *
   * A page that names a screen is the portal, because Next owns the `/portal/*` URL and Rust does not — so the screen
   * travels as data. The error boundary says so explicitly, because the URL it renders at is the one that FAILED and
   * must not be resolved to a screen. Anything else is the public site, whose own router reads the URL.
   */
  const app = error ? 'site-error' : screen ? 'portal' : 'site'
  const [bootError, setBootError] = useState<string | null>(null)
  /**
   * The container THIS component owns.
   *
   * A ref, not a lookup: see the note above, and `shell::start_in`.
   */
  const host = useRef<HTMLDivElement>(null)

  useEffect(() => {
    /**
     * The module is booted once and mounted on EVERY run.
     *
     * THE BUG THIS SHAPE REPLACES: an earlier version guarded with a ref and a `disposed` flag, so React 19's second
     * development run returned early while the first had already been abandoned — every page on an empty container
     * forever, with no error anywhere. The rule is NOT "the second run must not execute"; it is **only the current run
     * may mount**. `bootRustUi` is idempotent (see `lib/rust-ui/mount.ts`) and the generation below is what makes an
     * obsolete run harmless: it cannot mount, and it cannot set an error on a page that is no longer here.
     */
    let cancelled = false
    void bootRustUi(
      () => import('@/lib/rust-ui/ui.js') as unknown as Promise<RustUiModule>,
      wasmPath,
    )
      .then((module) => {
        if (!cancelled && host.current) module.start_in(host.current)
      })
      .catch((cause: unknown) => {
        if (!cancelled) setBootError(cause instanceof Error ? cause.message : String(cause))
      })
    return () => {
      cancelled = true
    }
  }, [screen, scope, error, wasmPath])

  return (
    <>
      <div
        ref={host}
        /**
         * THE KEY IS THE APPLICATION — the screen for the portal, and the application's own name otherwise — so React
         * can never hand this node to a second screen. Rust destroys the application it replaces
         * (`yew_portal::MOUNTED`); this makes sure there is a fresh node to mount it into as well.
         */
        key={app === 'portal' ? screen : app}
        id="rust-ui"
        data-rust-app={app}
        data-rust-screen={screen ?? ''}
        data-rust-scope={scope ?? ''}
      />
      {screen === 'projects' ? <ProjectReactIslands /> : null}
      {screen === 'tech' ? <TechCockpitReactIslands /> : null}
      {screen === 'trace-record' ? <FlightRecorderReactIsland /> : null}
      {screen === 'design-lab' ? <UiLabReactIslands /> : null}
      {screen === 'property-admin' ? <OpsVideoReactIsland /> : null}
      {bootError ? (
        <p className="p-6 text-sm text-destructive" role="alert">
          The application could not start: {bootError}
        </p>
      ) : null}
    </>
  )
}
