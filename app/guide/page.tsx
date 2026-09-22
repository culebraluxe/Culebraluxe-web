import { YewApp } from '@/components/rust-ui/yew-app'

// ---------------------------------------------------------------------------
// /guide — YEW OWNS THIS ROUTE NOW.
//
// The page is a Yew component (`rust/ui/src/yew_views/guide.rs`) driven by the MVI reducer. The nine sections are
// `view::GUIDE_SECTIONS` — the table the string renderer read — and every card arrives from `guide_item` through the
// public page payload. The jump nav is nine in-page anchors, so the index still works with no JavaScript at all, and the
// closing invitation is a router link rather than a page load. This file only puts the mount point on the page.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <YewApp />
    </div>
  )
}
