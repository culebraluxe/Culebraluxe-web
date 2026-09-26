import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// /buyers — YEW OWNS THIS ROUTE NOW.
//
// The screen is not painted by TypeScript and not by a string renderer: the wasm module boots the Yew application,
// the Yew router owns the URL, and the MVI reducer that always owned the state still does. What this file contributes is
// the mount point — the component holds no state, fetches nothing and listens for nothing.
//
// The screen, its search contract and its controls live in `rust/ui/src/yew_views/buyers.rs`.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <RustUi />
  )
}
