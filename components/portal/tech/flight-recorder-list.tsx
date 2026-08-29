import Link from "next/link"
import { GitBranch, Clock, Activity, ChevronRight, Layers } from "lucide-react"

import type { WorkflowSummary } from "@/workflow_app/read-service"

// FLIGHT-RECORDER — the engineering entry point to the Workflow Runtime
// Inspector. Lists workflow executions; each card opens the Inspector, which
// overlays the design-time topology with command/domain/node/transition trace
// evidence, the causal graph, and the machine/human/external process profile.

function statusTone(outcome: string | null, status: string): string {
  if (outcome === "cancelled") return "bg-black/5 text-black/55"
  if (outcome === "failed" || outcome === "conflict" || status === "error")
    return "bg-red-50 text-red-700"
  if (outcome === "completed") return "bg-emerald-50 text-emerald-700"
  return "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]"
}

export function FlightRecorderList({
  configured,
  summaries,
  goldenQa,
}: {
  configured: boolean
  summaries: WorkflowSummary[]
  goldenQa?: { instanceId: string; dealId: string; property: string | null; client: string | null } | null
}) {
  if (!configured) {
    return (
      <p className="text-sm font-light text-black/55">
        The workflow engine is not configured for this environment.
      </p>
    )
  }
  return (
    <div className="space-y-3">
      {goldenQa && (
        <div className="block rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/40 bg-[var(--portal-gold-pale)] p-5">
          <div className="flex items-start justify-between gap-4">
            <div>
              <div className="text-[10px] font-light uppercase tracking-[0.22em] text-[var(--portal-gold)]">
                Golden QA Transaction
              </div>
              <h2 className="mt-1.5 font-serif text-lg font-light text-[var(--portal-navy)]">
                {goldenQa.property ?? "Flight Recorder Golden Purchase"}
              </h2>
              {goldenQa.client && (
                <p className="mt-1 text-xs font-light text-black/55">{goldenQa.client}</p>
              )}
            </div>
            <Link
              href={`/portal/tech/flight-recorder/${goldenQa.instanceId}`}
              className="flex items-center gap-1 rounded-md bg-[var(--portal-navy)] px-3 py-1.5 text-xs font-medium text-white hover:bg-black/80"
            >
              <Layers className="h-3.5 w-3.5" />
              Open Flight Recorder
            </Link>
          </div>
        </div>
      )}
      {summaries.length === 0 ? (
        <p className="text-sm font-light text-black/55">
          No workflow executions recorded yet. Start a workflow to see its
          runtime evidence here.
        </p>
      ) : (
        summaries.map((s) => (
          <div
          key={s.instanceId}
          className="block rounded-[var(--portal-panel-radius)] portal-glass-panel p-5 transition-colors hover:border-[var(--portal-blue-gray)]/50"
        >
          <div className="flex items-start justify-between gap-4">
            <div>
              <div className="flex items-center gap-2 text-[10px] font-light uppercase tracking-[0.22em] text-black/45">
                <GitBranch className="h-3.5 w-3.5 text-[var(--portal-blue-gray)]" />
                {s.workflowName} · v{s.workflowVersion}
              </div>
              <h2 className="mt-1.5 font-serif text-lg font-light text-[var(--portal-navy)]">
                {s.propertyName ?? "Execution"}
              </h2>
            </div>
            <span
              className={`rounded-full px-3 py-1 text-xs font-light capitalize ${statusTone(s.outcome, s.status)}`}
            >
              {s.outcome ?? s.status}
            </span>
          </div>
          <div className="mt-3 flex items-center justify-between gap-3 text-xs font-light text-black/50">
            <span className="flex items-center gap-1.5">
              <Clock className="h-3.5 w-3.5" />
              {s.instanceId}
            </span>
            <span className="flex items-center gap-2">
              <Link
                href={`/portal/tech/flight-recorder/${s.instanceId}`}
                className="flex items-center gap-1 rounded-md bg-[var(--portal-navy)] px-3 py-1.5 text-xs font-medium text-white hover:bg-black/80"
              >
                <Layers className="h-3.5 w-3.5" />
                Open Flight Recorder
                <ChevronRight className="h-3.5 w-3.5" />
              </Link>
              <Link
                href={`/portal/runtime-inspector/${s.instanceId}`}
                className="flex items-center gap-1 rounded-md border border-black/10 px-2.5 py-1.5 text-xs text-black/50 hover:border-black/25 hover:text-black/70"
              >
                <Activity className="h-3.5 w-3.5" />
                Runtime Inspector
              </Link>
            </span>
          </div>
        </div>
      )))}
    </div>
  )
}
