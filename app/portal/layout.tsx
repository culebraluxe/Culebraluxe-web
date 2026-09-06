import type { ReactNode } from "react"
import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { toPortalActorSnapshot } from "@/lib/auth/actor-snapshot"
import { OperatingShell } from "@/components/portal/operating-shell"
import {
  getPortalPaletteClients,
  getPortalPaletteDeals,
} from "@/db/portal-palette"

export const dynamic = "force-dynamic"

// AUTH-02 authoritative Portal guard (server component). The middleware is only
// the cheap Edge first gate; THIS is where portal.read is enforced against the
// DB-backed canonical projection. Unauthenticated → /login; authenticated but
// missing portal.read (or inactive/unmapped) → /login/unauthorized.
//
// UI-01: the rendering shell is the operating-surface shell (NEXUS | OPS |
// TECH | SUPPORT + contextual navigation) — the persistent left rail is gone.
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

  // The shell command palette only needs lightweight labels/ids. Never hydrate
  // full Client/Deal aggregates here: PortalLayout executes for every portal route.
  const [clients, deals] = await Promise.all([
    getPortalPaletteClients(),
    getPortalPaletteDeals(),
  ])

  return (
    <OperatingShell actor={actor} clients={clients} deals={deals}>
      {children}
    </OperatingShell>
  )
}
