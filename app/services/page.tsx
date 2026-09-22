import { YewApp } from '@/components/rust-ui/yew-app'

// ---------------------------------------------------------------------------
// /services — YEW OWNS THIS ROUTE NOW.
//
// The page is a Yew component (`rust/ui/src/yew_views/services.rs`), driven by the same MVI reducer and the same content
// tables the string renderer read. This file only puts the mount point on the page.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <YewApp />
    </div>
  )
}

