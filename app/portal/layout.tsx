import type { ReactNode } from "react"
import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { toPortalActorSnapshot } from "@/lib/auth/actor-snapshot"

export const dynamic = "force-dynamic"

// AUTH-02 authoritative Portal guard (server component). The middleware is only
// the cheap Edge first gate; THIS is where portal.read is enforced against the
// DB-backed canonical projection. Unauthenticated → /login; authenticated but
// missing portal.read (or inactive/unmapped) → /login/unauthorized.
//
// THE CHROME IS RUST. The frame (top nav and rail) is drawn by the Yew portal app around every screen - a screen without a
// Yew component yet renders its markup inside that chrome (`yew_portal::StringBody`). The React OperatingShell is gone.
//
// THE ACTOR IS HANDED OVER AS JSON, and why it has to be: the nav decides which operating worlds and which rail items to
// offer from the same rules the React shell used (`lib/navigation/registry.ts`), and those rules live in Rust now. So
// the projection is written into the page and `rust/ui/src/shell.rs` adopts it on every mount path — the string host
// does not go through `start`, so it cannot be an argument to one entry point.
//
// THE GUARD STAYS AND IS NOT DECORATION: it is the boundary. The nav hiding a link is not a gate, and the projection is
// cosmetic — every route re-checks authority server-side.
export default async function PortalLayout({
  children,
}: {
  children: ReactNode
}) {
  const result = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "portal.read",
  )
  if (!result.ok) redirect(result.redirectTo)

  // Cosmetic UI projection only — hiding buttons is never the security boundary.
  const actor = toPortalActorSnapshot(result.actor)

  return (
    <>
      <script
        id="rust-actor"
        type="application/json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(actor) }}
      />
      {children}
    </>
  )
}
