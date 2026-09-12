import { Suspense } from "react"
import { redirect } from "next/navigation"

import { EngineeringLinePage } from "@/components/portal/tech/grok-engineering-line"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const dynamic = "force-dynamic"

// GROK REFERENCE — the Engineering Line mockup, from the fixture stubs only
// (`components/portal/tech/grok-engineering-line/fixture.ts`). No database, no
// engine: it exists so the Bench -> Line -> Recorder shape can be CLICKED and
// compared against what /portal/tech renders today. Same freeze rule as the
// Flight Recorder reference at /portal/tech/grok: this is the untouched mock, so
// wiring the real loader means editing `loadEngineeringLine()` or building the
// consolidated screen, not bending the reference. Requires tech.access (ROOT only).
export default async function GrokEngineeringLineRoute() {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  return (
    <div className="h-screen overflow-hidden bg-[#0b1220]">
      <Suspense
        fallback={
          <div className="grid h-screen place-items-center bg-[#0b1220] text-sm text-slate-400">
            Loading the line…
          </div>
        }
      >
        <EngineeringLinePage />
      </Suspense>
    </div>
  )
}
