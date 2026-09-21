'use client'

import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// The portal host: the portal screens rendered from Rust.
//
// Thin on purpose — the shell, the fetch and the effect handling live in the shared host, and this page only says
// which area it is mounting (the portal rows route, behind the session) and which screen opens first.
//
// It is scaffolding for the port, not the destination: as each portal screen is finished, its real `/portal/*` route
// becomes the host for that screen and this preview stays behind as the place to exercise the shell alone.
// ---------------------------------------------------------------------------

export default function RustPreviewPage() {
  return (
    <div className="min-h-screen bg-background">
      <RustUiHost rowsPath="/api/portal/rust-ui/rows" start="dashboard" />
    </div>
  )
}
