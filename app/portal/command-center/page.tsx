import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// CONVERTED TO YEW (screen: command-center, surface Tech).
//
// The route is unchanged; the screen is not. It rendered a TypeScript component with its own state
// (15 interactive hooks); it now renders the Rust screen, fed by the portal rows route.
//
// HONEST NOTE ON PARITY: this crossed over before the Rust body had those controls, on instruction
// that the conversion comes first and the gaps are worked afterwards. What is missing is named at
// docs/layers/UI.md rather than implied by silence - the rows are here, the behaviour is the
// follow-up. Scripts: scripts/ui-flip-readiness.mjs for the count.
// ---------------------------------------------------------------------------

export default function Page() {

  return (
    <RustUi screen="command-center" />
  )
}
