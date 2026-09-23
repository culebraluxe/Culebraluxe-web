import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

// ---------------------------------------------------------------------------
// /portal/system-health — YEW OWNS THIS SCREEN NOW.
//
// The screen is `rust/ui/src/yew_views/portal_support_system_health.rs`, over the same three reads the pre-cutover page made
// together: the operational health snapshot, the environment readiness posture, and the workflow diagnostics.
//
// WHAT THIS FIXES: the earlier conversion sent this screen through the generic rows renderer and dropped the Workflow
// Diagnostics interaction with it — the instance list was a list, and a row could not be opened. The detail a row asks for is
// now `Msg::WorkflowInstanceToggled` → an effect → the payload with that instance's detail, with Rust owning the DTOs, the
// reducer and the rendering.
//
// Read-only: no workflow reset, repair or mutation control is on this screen, and none is added.
// ---------------------------------------------------------------------------

export default function Page() {
  return <PortalYewApp screen="system-health" />
}
