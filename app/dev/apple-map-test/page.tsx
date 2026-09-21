import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// CONVERTED TO RUST (screen: dev-apple-map-test) — a screen with no read model, which says so.
//
// The Rust table marks this screen deferred with its reason, so the screen states why it has no rows instead of
// showing a blank list, and it asks the host for nothing. The route is the Rust host like every other converted one.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <RustUiHost rowsPath="/api/rust-ui/public-rows" start="dev-apple-map-test" />
    </div>
  )
}
