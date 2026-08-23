import type { ReactNode } from "react"
import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { toPortalActorSnapshot } from "@/lib/auth/actor-snapshot"
import { OperatingShell } from "@/components/portal/operating-shell"
import { getClients } from "@/db/clients"
import { getDeals } from "@/db/deals"

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
  const [clients, deals] = await Promise.all([getClients(), getDeals()])

  return (
    <OperatingShell
      actor={actor}
      clients={clients.map((client) => ({
        id: client.id,
        name: client.displayName,
      }))}
      deals={deals.map((deal) => ({
        id: deal.id,
        name: deal.propertyName,
        client: deal.clientName,
      }))}
    >
      {children}
    </OperatingShell>
  )
}
