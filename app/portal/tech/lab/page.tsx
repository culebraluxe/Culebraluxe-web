import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// TECH LAB — what Rust does with layout, on this application's own design tokens.
//
// A real page, not a preview: it lives at a real route under TECH and it is the screen the captain asked for. The
// markup is rendered entirely by `rust/ui/src/view.rs` (`tech_lab`), which is the point — every class on it is one
// this application already defines, nothing is positioned with an inline style or a magic number, and the stylesheet
// decides how it looks.
//
// The rows endpoint is not involved: this screen has no read model, so the host asks for nothing and the shell paints
// the screen's own body.
// ---------------------------------------------------------------------------

export default function TechLabPage() {
  return (
    <RustUi screen="tech-lab" />
  )
}
