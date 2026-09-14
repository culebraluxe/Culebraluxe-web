import { Suspense } from "react"
import { redirect } from "next/navigation"

import { FlightRecorderConsoleShell } from "@/components/portal/tech/flight-recorder-console-shell"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { dbTargetInfo } from "@/db/database-gateway"
import { latestForgeInstanceForStory } from "@/db/forge-engine-task-execution"
import { isProcessInstanceId } from "@/workflow_app/flight-recorder-read"

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

  // A CARD KNOWS ITS STORY, NOT ITS INSTANCE — so accept a story id and resolve it here.
  //
  // A story gets a fresh process instance every attempt, which is why the board hands over a story
  // id and used to append a "-demo" placeholder to pretend it was an instance. The screen then
  // asked the database for a trace belonging to a string that had never been an instance, and the
  // only answers available were 503 (before) and 400 (after the id guard). Resolving story ->
  // latest instance is the behaviour the operator remembers: hand it a story number, it loads.
  // The URL is rewritten to the canonical instance UUID so the address bar always names the exact
  // trace being read, and a story with no run says so instead of opening an empty shell.
  if (!isProcessInstanceId(instanceId)) {
    const latest = await latestForgeInstanceForStory(instanceId).catch(() => null)
    if (latest && isProcessInstanceId(latest)) {
      redirect(`/portal/tech/flight-recorder/${latest}`)
    }
    return (
      <div className="h-[calc(100vh-8rem)] overflow-hidden bg-[#0b1220]">
        <div className="grid h-full place-items-center px-6 text-center">
          <div className="max-w-xl rounded-lg border border-white/10 px-6 py-5 text-sm text-slate-300">
            <p className="mb-2 font-mono text-[11px] uppercase tracking-[0.14em] text-[#c6a15b]">
              No engine instance for {instanceId}
            </p>
            <p className="leading-relaxed">
              Nothing in the engine ledger records a run for this id, so there is no trace to read.
              The cockpit shows a Flight Recorder link only for stories the engine has actually
              executed; a demo card in the queues board has fixture data behind it and no run.
            </p>
          </div>
        </div>
      </div>
    )
  }

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
