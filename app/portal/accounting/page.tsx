import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// /portal/accounting — YEW OWNS THIS SCREEN NOW.
//
// The generic rows renderer is gone from this route. The screen is
// `rust/ui/src/yew_views/portal_accounting_dashboard.rs`, its data comes from `/api/portal/rust-ui/page` (which reads the
// Rust `/v1/accounting/dashboard` projection), and every figure on it was computed by Postgres on `numeric`: the amounts
// arrive as decimal strings and nothing on this screen adds or divides money.
//
// This file only puts the mount point on the page.
// ---------------------------------------------------------------------------

export default function Page() {
  return <RustUi screen="accounting" />
}
