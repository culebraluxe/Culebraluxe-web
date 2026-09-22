import { YewApp } from '@/components/rust-ui/yew-app'

// ---------------------------------------------------------------------------
// / — YEW OWNS THIS ROUTE NOW.
//
// The landing page is a Yew component (`rust/ui/src/yew_views/home.rs`) driven by the MVI reducer: its hero and every
// band below it come from the page payload, exactly as the string renderer read them. This file only puts the mount
// point on the page.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <YewApp />
    </div>
  )
}

