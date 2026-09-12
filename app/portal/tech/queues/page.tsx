import { Suspense } from "react"
import { redirect } from "next/navigation"

import { EngineeringQueuesPage } from "@/components/portal/tech/engineering-line"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const dynamic = "force-dynamic"

// ENGINEERING QUEUES — the working screen.
//
// Four queues and a stats strip: WORK BENCH (human) · ENGINE READY · RUNNING ·
// RESULTS (finished attempts, where the outcome DONE / ERROR / HOLD is a badge on
// the row). Double-clicking a RESULTS card opens that attempt in the Flight
// Recorder, which is the drill-down this screen already owns.
//
// Iteration 1 is LAYOUT with static data: the numbers are real (read from PROD on
// 2026-09-12) but the cards come from the fixture, so the mechanic can be judged
// before anything is wired. The single seam is loadEngineeringQueues().
//
// When this earns its place it REPLACES /portal/tech, and command-center +
// command-console + runtime-inspector come down with it. Grok's Engineering Line
// reference at /portal/tech/line stays frozen and untouched for comparison.
export default async function EngineeringQueuesRoute() {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  return (
    <Suspense
      fallback={
        <div className="grid h-screen place-items-center bg-[#0b1220] text-sm text-slate-400">
          Loading queues…
        </div>
      }
    >
      <EngineeringQueuesPage />
    </Suspense>
  )
}
