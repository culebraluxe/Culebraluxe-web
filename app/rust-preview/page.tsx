import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// The public preview: the "main front" screens rendered from Rust.
//
// It is a separate page from the portal preview on purpose — this one sits outside `/portal`, so it loads with the
// public site's own layout and can be opened by anyone. Its rows come from `/api/rust-ui/public-rows`, which is
// unauthenticated because the data behind it is already published on this site; it can reach nothing else.
//
// It opens on the listings rather than the home page: the listings are where the data is. The URL is its own route in
// the Yew router (`/rust-preview`), which is what lets the page pass no screen at all — the router owns the URL.
// ---------------------------------------------------------------------------

export const dynamic = 'force-dynamic'

export default function RustPreviewPage() {
  return (
    <RustUi />
  )
}
