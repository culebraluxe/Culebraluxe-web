import type { ReactNode } from "react"
import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { toPortalActorSnapshot } from "@/lib/auth/actor-snapshot"
import { PortalHeader } from "@/components/portal/portal-header"
import { PortalSidebar } from "@/components/portal/portal-sidebar"

export const dynamic = "force-dynamic"

// AUTH-02 authoritative Portal guard (server component). The middleware is only
// the cheap Edge first gate; THIS is where portal.read is enforced against the
// DB-backed canonical projection. Unauthenticated → /login; authenticated but
// missing portal.read (or inactive/unmapped) → /login/unauthorized.
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
    <div className="min-h-screen bg-[var(--portal-bg)] text-[var(--portal-text)]">
      <div className="flex min-h-screen">
        <PortalSidebar actor={actor} />

        <div className="min-w-0 flex-1">
          <PortalHeader actor={actor} />

          <main className="px-6 py-8 lg:px-10 lg:py-10 xl:px-14">
            {children}
          </main>
        </div>
      </div>
    </div>
  )
}
