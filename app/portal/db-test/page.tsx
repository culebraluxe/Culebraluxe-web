import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

// ---------------------------------------------------------------------------
// /portal/db-test — YEW OWNS THIS SCREEN NOW.
//
// The screen is `rust/ui/src/yew_views/portal_support_db_test.rs`. It answers what the pre-cutover page answered — is the
// database reachable, how many clients, and who they are — from the same server-side client read, presented instead of
// dumped as JSON. Read-only: no controls, no writes.
// ---------------------------------------------------------------------------

export default function Page() {
  return <PortalYewApp screen="db-test" />
}
