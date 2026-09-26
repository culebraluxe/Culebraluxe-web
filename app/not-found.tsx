import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// THE 404, RENDERED BY RUST.
//
// Next renders this at any URL it has no route for. The Yew router resolves that unknown path to its own catch-all and
// draws the page inside the application's own header and footer — so the 404 is the same site as every other page,
// drawn once, in one place (`rust/ui/src/yew_router.rs`, the `NotFound` component).
//
// This was the last page using the React `SiteHeader` and `SiteFooter`, and both are deleted with it: there is one site
// chrome now and it is Rust's.
// ---------------------------------------------------------------------------

export default function NotFound() {
  return <RustUi />
}
