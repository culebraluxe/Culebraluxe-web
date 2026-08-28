import { Suspense } from "react"
import { redirect } from "next/navigation"

import { FlightRecorderPage } from "@/components/portal/tech/grok-flight-recorder/FlightRecorderPage"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const dynamic = "force-dynamic"

// GROK REFERENCE — the base mockup Grok designed with the FAKE fixture stubs.
// This is the untouched frozen reference (same dark full-screen console, fake
// deal trace from grok/flight-recorder/src/fixture.ts) so it can be compared
// side-by-side against the real engine-backed Flight Recorder console at
// /portal/tech/flight-recorder/[instanceId]. Requires tech.access (ROOT only).
export default async function GrokFlightRecorderPage() {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "tech.access",
  )
  if (!access.ok) redirect(access.redirectTo)

  return (
    <div className="h-screen overflow-hidden bg-[#0b1220]">
      <Suspense
        fallback={
          <div className="grid h-screen place-items-center bg-[#0b1220] text-sm text-slate-400">
            Loading trace…
          </div>
        }
      >
        <FlightRecorderPage />
      </Suspense>
    </div>
  )
}
