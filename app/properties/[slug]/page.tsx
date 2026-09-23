import { YewApp } from '@/components/rust-ui/yew-app'

// ---------------------------------------------------------------------------
// /properties/:slug — YEW OWNS THE DYNAMIC PROPERTY ROUTE.
//
// The Yew router reads the slug from the browser URL and mounts site-property-detail with that
// scope. The reducer owns gallery/lightbox state and the page effect loads the scoped property.
// There is no second React carousel and no string-renderer interaction shim.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <YewApp />
    </div>
  )
}
