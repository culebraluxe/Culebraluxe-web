import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// FLIPPED TO RUST (screen: site-home, surface Site).
//
// The route is unchanged; the screen is not. It was a TypeScript page fetching its own data for a TypeScript
// component. It is now the Rust host: the same read models arrive through the public rows route and
// rust/ui/src/view.rs paints the screen.
//
// It qualified because its TypeScript body has no interaction to lose - no state, form, dialog, filter or paging
// control - so there is nothing the Rust body can fail to reproduce. Screens whose TypeScript carries behaviour stay
// in TypeScript until the Rust body has its own controls. scripts/ui-flip-readiness.mjs prints the split at any time.
// ---------------------------------------------------------------------------

export default function Page() {

  return (
    <div className="min-h-screen bg-background">
      <RustUiHost rowsPath="/api/rust-ui/public-rows" start="site-home" />
    </div>
  )
}
