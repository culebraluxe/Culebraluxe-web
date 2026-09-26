import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// /sellers — YEW OWNS THIS ROUTE NOW.
//
// The page is a Yew component (`rust/ui/src/yew_views/sellers.rs`) driven by the MVI reducer. Its seven sections come
// from the shared tables the string renderer also reads. This file only puts the mount point on the page.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <RustUi />
  )
}
