import { YewApp } from '@/components/rust-ui/yew-app'

// ---------------------------------------------------------------------------
// /account — YEW OWNS THIS ROUTE. Guest sign-in and the signed-in guest (`rust/ui/src/yew_views/account.rs`).
// This file only puts the mount point on the page.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <YewApp />
    </div>
  )
}
