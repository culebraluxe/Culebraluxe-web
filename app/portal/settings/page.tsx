import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// /portal/settings — YEW OWNS THIS SCREEN NOW.
//
// The Security landing screen: `rust/ui/src/yew_views/portal_support_security.rs`. It reads the same two server-side
// projections the pre-cutover page read (security counts and break-glass readiness) and shows the three navigation cards to
// Users, Roles and Authorities. Read-only, and no credential — no secret, no hash, no root id — crosses the boundary.
//
// The three child routes are untouched and still render their own screens.
// ---------------------------------------------------------------------------

export default function Page() {
  return <RustUi screen="security" />
}
