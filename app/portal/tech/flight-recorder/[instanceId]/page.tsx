import { redirect } from "next/navigation"

import { FlightRecorderConsoleShell } from "@/components/portal/tech/flight-recorder-console-shell"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const dynamic = "force-dynamic"

// PORTAL — Flight Recorder console for a single workflow instance. A parallel
// facelift route: it loads the SAME real engine evidence the Runtime Inspector
// uses (via /api/portal/runtime-inspector/:id) and renders it through the
// Flight Recorder console read-model. The Runtime Inspector page is untouched.
export default async function FlightRecorderConsolePage({
  params,
}: {
  params: Promise<{ instanceId: string }>
}) {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "tech.access",
  )
  if (!access.ok) redirect(access.redirectTo)

  const { instanceId } = await params
  return (
    <div className="h-screen overflow-hidden bg-[#0b1220]">
      <FlightRecorderConsoleShell instanceId={instanceId} />
    </div>
  )
}
