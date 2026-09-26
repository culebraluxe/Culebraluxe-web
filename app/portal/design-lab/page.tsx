import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// /portal/design-lab — THE ONE UI LAB.
//
// Yew owns the comparison shell and native Rust gallery. The preserved React/TypeScript gallery and
// Framer MVI lab mount as bounded React portals inside sibling tabs; neither creates a second React root
// or takes application ownership away from Yew.
// ---------------------------------------------------------------------------

export default function DesignLabPage() {
  return <RustUi screen="design-lab" />
}
