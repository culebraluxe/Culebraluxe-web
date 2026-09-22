import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

// ---------------------------------------------------------------------------
// /portal/accounting/receivables — YEW OWNS THIS SCREEN NOW.
//
// The screen is `rust/ui/src/yew_views/portal_accounting_receivables.rs`. Creating a receivable and marking one paid are both
// Rust commands (`/v1/accounting/receivables` and `/v1/accounting/receivables/{id}/paid`); the row dates and the form's
// fields live in the reducer. This file holds no state and runs no server action.
// ---------------------------------------------------------------------------

export default function Page() {
  return <PortalYewApp screen="accounting-receivables" />
}
