import { YewApp } from '@/components/rust-ui/yew-app'

// ---------------------------------------------------------------------------
// /favorites — YEW OWNS THIS ROUTE NOW.
//
// Saved properties (`rust/ui/src/yew_views/favorites.rs`): the published listings, picked out by what this device has
// saved with the heart on a card. This file only puts the mount point on the page.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <YewApp />
    </div>
  )
}
