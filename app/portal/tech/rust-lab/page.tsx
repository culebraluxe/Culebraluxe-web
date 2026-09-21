import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// RUST LAB — the control vocabulary this crate renders, wired to the model.
//
// The sibling of `/portal/design-lab`, which stays TypeScript on purpose: that one catalogues React components and is
// about those components. This one is about the controls `rust/ui` owns — text field, dropdown, switch, tabs, pager —
// and it is meant to be operated: filter, switch, page and watch the model-state panel at the bottom follow.
//
// The rows endpoint is asked for nothing here. The screen renders its own body from a constant so the lab tests the
// controls rather than the network; an empty rows answer is the honest one.
// ---------------------------------------------------------------------------

export default function RustLabPage() {
  return (
    <div className="min-h-screen bg-background">
      <RustUiHost rowsPath="/api/portal/rust-ui/rows" start="rust-lab" />
    </div>
  )
}
