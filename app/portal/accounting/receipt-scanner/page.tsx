import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// /portal/accounting/receipt-scanner — YEW OWNS THIS SCREEN NOW.
//
// The screen is `rust/ui/src/yew_views/portal_accounting_receipt_scanner.rs`. It keeps the FAKE V1 demonstration exactly:
// four deterministic seeds, a review step, and saving through the same Rust expense command the Expenses form uses. No OCR
// is called, here or anywhere else on this path — the Apple Vision tool stays where it is.
// ---------------------------------------------------------------------------

export default function Page() {
  return <RustUi screen="accounting-receipt-scanner" />
}
