import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// /about — YEW OWNS THIS ROUTE NOW.
//
// The page is a Yew component (`rust/ui/src/yew_views/about.rs`) driven by the MVI reducer. Its copy is literal, as it is
// in the page it replaces, and it lives in the shared tables the string renderer also reads. This file only puts the
// mount point on the page.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <RustUi />
  )
}
