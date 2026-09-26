import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// /portal/accounting/expenses — YEW OWNS THIS SCREEN NOW.
//
// The screen is `rust/ui/src/yew_views/portal_accounting_expenses.rs`. The list and its category breakdown come from the
// Rust service (`/v1/accounting/expenses` and `/v1/accounting/expense-categories`), and creating an expense is a Rust
// command: this route posts nothing, holds no state, and runs no server action. The form's fields live in the reducer.
// ---------------------------------------------------------------------------

export default function Page() {
  return <RustUi screen="accounting-expenses" />
}
