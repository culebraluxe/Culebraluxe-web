import { Suspense } from "react"
import { redirect } from "next/navigation"

import { FlightRecorderConsoleShell } from "@/components/portal/tech/flight-recorder-console-shell"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const dynamic = "force-dynamic"

// PORTAL — Flight Recorder console for a single workflow instance. Renders the
// completed four-view console (Timeline / Causality / System Swimlane / Raw
// Events) via FlightRecorderConsoleShell, which loads the canonical Flight
// Recorder transaction read model (/api/portal/flight-recorder/:id). Runtime
// Inspector is a separate engineering diagnostic.
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
    // Height accounts for the portal shell (navy bar + TECH submenu + main
    // padding) so the dark console fills the visible area instead of overflowing
    // the viewport and clipping its controls.
    <div className="h-[calc(100vh-8rem)] overflow-hidden bg-[#0b1220]">
      <Suspense
        fallback={
          <div className="grid h-full place-items-center bg-[#0b1220] text-sm text-slate-400">
            Loading trace…
          </div>
        }
      >
        <FlightRecorderConsoleShell instanceId={instanceId} />
      </Suspense>
    </div>
  )
}
