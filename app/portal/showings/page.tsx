import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// FLIPPED TO RUST (screen: showings, surface Core).
//
// The route is unchanged; the screen is not. It was a TypeScript page fetching its own data for a TypeScript
// component. It is now the Yew portal app: the same read models arrive through the portal rows route and
// the body this crate already renders is drawn inside the Yew chrome (yew_portal::StringBody).
//
// It qualified because its TypeScript body has no interaction to lose - no state, form, dialog, filter or paging
// control - so there is nothing the Rust body can fail to reproduce. Screens whose TypeScript carries behaviour stay
// in TypeScript until the Rust body has its own controls. scripts/ui-flip-readiness.mjs prints the split at any time.
// ---------------------------------------------------------------------------

export default function Page() {

  return (
    <RustUi screen="showings" />
  )
}
