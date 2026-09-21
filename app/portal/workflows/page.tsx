import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// FLIPPED TO RUST (screen: workflows).
//
// The route is unchanged; the screen is not. What used to be a TypeScript page fetching its own data and handing it to
// a TypeScript component is now the Rust host, which reads the same data through the portal rows route and paints the
// screen from rust/ui/src/view.rs.
//
// This page qualified because it is a read-only list: its TypeScript body carries no state, no form, no dialog and no
// paging control, so there is nothing the Rust screen can fail to reproduce. Screens that carry interaction stay in
// TypeScript until the Rust body has its own controls. The list is in docs/layers/UI.md.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <RustUiHost rowsPath="/api/portal/rust-ui/rows" start="workflows" />
    </div>
  )
}
