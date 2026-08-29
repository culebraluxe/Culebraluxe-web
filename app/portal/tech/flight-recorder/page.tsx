import { redirect } from "next/navigation"
import Link from "next/link"

import { FlightRecorderList } from "@/components/portal/tech/flight-recorder-list"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { getWorkflowSummaries } from "@/workflow_app/read-service"
import { engineConfigured } from "@/workflow_app/engine-client"
import { findGoldenQaWorkflowInstance } from "@/workflow_app/flight-recorder-read"

export const dynamic = "force-dynamic"

// PORTAL — Flight Recorder / Workflow Runtime Inspector entry point. Requires
// tech.access (ROOT only). Lists workflow executions and opens the Runtime
// Inspector (design-time topology overlay + causal graph + machine/human/
// external process profile) for any instance.
export default async function FlightRecorderPage() {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "tech.access",
  )
  if (!access.ok) redirect(access.redirectTo)

  const configured = engineConfigured()
  const summaries = configured ? await getWorkflowSummaries() : []
  const goldenQa = await findGoldenQaWorkflowInstance()

  return (
    <div className="mx-auto max-w-5xl space-y-5 p-4 lg:p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-serif text-2xl font-light text-[var(--portal-navy)]">
            Flight Recorder
          </h1>
          <p className="mt-1 max-w-2xl text-sm font-light leading-6 text-black/55">
            Workflow execution evidence — the Runtime Inspector overlays each
            instance&apos;s design-time topology with command / domain / node /
            transition trace data, a causal graph, and a machine / human /
            external process profile.
          </p>
        </div>
        <Link
          href="/portal/workflows"
          className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]/60 hover:text-[var(--portal-navy)]"
        >
          ← Workflows
        </Link>
      </div>
      <FlightRecorderList
        configured={configured}
        summaries={summaries}
        goldenQa={goldenQa}
      />
    </div>
  )
}
