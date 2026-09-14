import { Suspense } from "react"
import { redirect } from "next/navigation"

import { FlightRecorderConsoleShell } from "@/components/portal/tech/flight-recorder-console-shell"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { dbTargetInfo } from "@/db/database-gateway"

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

  // WHICH DATABASE IS THIS SCREEN READING?
  //
  // The engine writes to the environment its own run declares (Forge runs against PROD); this
  // page reads whichever environment IT was deployed as. A local dev server declares
  // APP_ENV=development and therefore shows DEV while the live engine history sits in PROD -
  // the operator sees an empty recorder and concludes the engine did nothing, or that they are
  // in a parallel dimension. They are. So the screen says out loud which database it reads;
  // `dbTargetInfo` reports an undeclared environment rather than throwing, because this is
  // exactly the question an operator asks when they suspect the environment is wrong.
  const env = dbTargetInfo()
  const envLabel =
    env.target === 'prod' ? 'PRODUCTION' : env.target === 'dev' ? 'DEVELOPMENT' : 'UNDECLARED'

  return (
    <div className="bg-[#0b1220]">
      <div className="flex items-center justify-between gap-3 border-b border-white/10 px-3 py-1.5 text-[10px] uppercase tracking-[0.14em] text-slate-400">
        <span>Flight Recorder · {instanceId.slice(0, 8)}</span>
        <span className={env.target === 'prod' ? 'text-[#c6a15b]' : 'text-amber-400/90'}>
          reading {envLabel} · {env.neonBranch ?? 'branch unknown'} · declared by{' '}
          {env.declaredBy ?? 'nothing'}
        </span>
      </div>
      <div className="h-[calc(100vh-10rem)] overflow-hidden">
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
    </div>
  )
}
