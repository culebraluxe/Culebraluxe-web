import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

// ---------------------------------------------------------------------------
// /portal/design-lab — THE ONE UI LAB.
//
// Yew owns the comparison shell and the native Rust component gallery. The preserved React/TypeScript
// and Framer experiments are represented as sibling tabs and can be mounted as bounded islands later;
// this route does not resurrect the old generic Rust rows renderer.
// ---------------------------------------------------------------------------

export default function DesignLabPage() {
  return <PortalYewApp screen="design-lab" />
}
