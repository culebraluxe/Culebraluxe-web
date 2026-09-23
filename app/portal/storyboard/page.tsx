import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

// ---------------------------------------------------------------------------
// /portal/storyboard — native Yew/MVI read-only snapshot.
//
// The existing authenticated rows projection remains the data boundary. Yew owns the shell and story table; there is
// no React island and no client-side editor on this screen. Story detail stays at /portal/storyboard/[id].
// ---------------------------------------------------------------------------

export default function Page() {
  return <PortalYewApp screen="storyboard" />
}
