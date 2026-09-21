import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// The public host: the "main front" screens rendered from Rust.
//
// It is a separate page from the portal preview on purpose — this one sits outside `/portal`, so it loads with the
// public site's own layout and can be opened by anyone. The rows come from `/api/rust-ui/public-rows`, which is
// unauthenticated because the data behind it is already published on this site; it can reach nothing else.
//
// It is scaffolding for the port, not the destination: as each site screen is finished, its real route becomes the
// host for that screen and this stays behind as the place to exercise the shell alone.
// ---------------------------------------------------------------------------

export const dynamic = 'force-dynamic'

export default function RustPreviewPage() {
  return (
    <div className="min-h-screen bg-background">
      {/* It opens on the listings rather than the home page: the listings are where the data is, and the home page's
          editorial layout is not ported yet. Starting on a screen that would say "nothing to show yet" would make the
          shell look broken when it is only unfinished. */}
      <RustUiHost rowsPath="/api/rust-ui/public-rows" start="site-properties" />
    </div>
  )
}
