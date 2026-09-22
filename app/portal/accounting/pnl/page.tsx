import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

// ---------------------------------------------------------------------------
// /portal/accounting/pnl — YEW OWNS THIS SCREEN NOW.
//
// The screen is `rust/ui/src/yew_views/portal_accounting_pnl.rs`. The period lives in the reducer, Apply asks the Rust
// service for exactly that period through this route, and the statement echoes the period it covers. The generic rows
// renderer that used to draw this screen projected a hard-coded 2020-01-01 onwards.
// ---------------------------------------------------------------------------

export default function Page() {
  return <PortalYewApp screen="accounting-pnl" />
}
