import { YewApp } from '@/components/rust-ui/yew-app'

// ---------------------------------------------------------------------------
// /buyers — YEW OWNS THIS ROUTE NOW.
//
// The screen is no longer painted by the string renderer through `RustUiHost`: the wasm module boots a Yew application,
// the Yew router owns the URL, and the MVI reducer that always owned the state still does. What this file contributes is
// the mount point — the component holds no state, fetches nothing and listens for nothing.
//
// The screen, its search contract and its controls live in `rust/ui/src/yew_views/buyers.rs`.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <YewApp />
    </div>
  )
}
