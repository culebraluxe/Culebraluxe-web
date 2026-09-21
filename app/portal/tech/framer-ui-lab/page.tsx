import type { Metadata } from "next"
import { redirect } from "next/navigation"

import { FramerUiLab } from "@/components/portal/tech/framer-ui-lab"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const metadata: Metadata = {
  title: "Framer UI Lab",
}

export default async function FramerUiLabPage() {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "tech.access",
  )
  if (!access.ok) redirect(access.redirectTo)

  return <FramerUiLab />
}
