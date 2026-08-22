import type { ReactNode } from "react"
import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const dynamic = "force-dynamic"

// AUTH-02 settings guard (server component). The Portal layout already requires
// portal.read; this nested guard additionally requires settings.read so that
// viewer/agent accounts (portal.read but no settings.read) are denied
// /portal/settings* server-side, independent of any UI hiding.
export default async function SettingsLayout({
  children,
}: {
  children: ReactNode
}) {
  const result = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "settings.read",
  )
  if (!result.ok) redirect(result.redirectTo)

  return <>{children}</>
}
