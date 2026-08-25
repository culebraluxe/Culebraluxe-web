import Link from "next/link"
import { GitBranch, AlertCircle, CheckCircle2, Clock } from "lucide-react"

import type { WorkflowSummary } from "@/workflow_app/read-service"

function outcomePill(outcome: string | null, status: string) {
  if (outcome === "cancelled") return { label: "Cancelled", className: "bg-black/5 text-black/55" }
  if (outcome === "failed" || outcome === "conflict" || status === "error") {
    return { label: outcome ?? status, className: "bg-red-50 text-red-700" }
  }
  if (outcome === "completed") return { label: "Closed", className: "bg-emerald-50 text-emerald-700" }
  return { label: status, className: "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]" }
}

function WorkflowCard({ summary }: { summary: WorkflowSummary }) {
  const pill = outcomePill(summary.outcome, summary.status)

  return (
    <Link
      href={`/portal/workflows/${summary.instanceId}`}
      className="block rounded-[var(--portal-panel-radius)] portal-glass-panel p-6 transition-colors hover:border-[var(--portal-blue-gray)]/50"
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-2 text-[10px] font-light uppercase tracking-[0.22em] text-black/45">
            <GitBranch className="h-3.5 w-3.5 text-[var(--portal-blue-gray)]" />
            {summary.workflowName} · v{summary.workflowVersion}
          </div>
          <h2 className="mt-2 font-serif text-xl font-light text-[var(--portal-navy)]">
            {summary.propertyName ?? "Transaction"}
          </h2>
        </div>
        <span className={`rounded-full px-3 py-1 text-xs font-light capitalize ${pill.className}`}>
          {pill.label}
        </span>
      </div>

      <div className="mt-5 flex flex-wrap items-center gap-x-5 gap-y-2 text-sm font-light text-black/60">
        <span className="flex items-center gap-1.5">
          <Clock className="h-3.5 w-3.5" />
          {summary.activeMilestones.length > 0
            ? summary.activeMilestones.map(milestoneLabel).join(", ")
            : summary.responsibleParty ?? "No active milestone"}
        </span>
        {summary.blockerCount > 0 && (
          <span className="flex items-center gap-1.5 text-[var(--portal-blue-gray)]">
            <AlertCircle className="h-3.5 w-3.5" />
            {summary.blockerCount} blocker{summary.blockerCount > 1 ? "s" : ""}
          </span>
        )}
        {summary.openTaskCount > 0 && (
          <span className="flex items-center gap-1.5">
            <CheckCircle2 className="h-3.5 w-3.5" />
            {summary.openTaskCount} open task{summary.openTaskCount > 1 ? "s" : ""}
          </span>
        )}
      </div>
    </Link>
  )
}

function milestoneLabel(id: string) {
  return id.replace(/_/g, " ")
}

export function WorkflowsList({
  configured,
  summaries,
}: {
  configured: boolean
  summaries: WorkflowSummary[]
}) {
  return (
    <div>
      <header className="mb-8">
        <h1 className="font-serif text-3xl font-light text-[var(--portal-navy)]">Workflows</h1>
        <p className="mt-1 text-sm font-light text-black/55">
          Transaction orchestration for deals in motion.
        </p>
      </header>

      {!configured ? (
        <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-16 text-center">
          <GitBranch className="mx-auto h-9 w-9 text-[var(--portal-blue-gray)]" />
          <h2 className="mt-5 font-serif text-2xl font-light text-[var(--portal-navy)]">
            Workflow runtime not ready
          </h2>
          <p className="mx-auto mt-3 max-w-lg text-sm font-light leading-6 text-black/55">
            The workflow runtime shares the CulebraLuxe database. Apply the
            unified activation script (
            <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
              db/manual/2026-08-20_v4_crm14_workflow_activation.sql
            </code>
            ) to enable transaction workflows.
          </p>
        </section>
      ) : summaries.length === 0 ? (
        <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-16 text-center">
          <GitBranch className="mx-auto h-9 w-9 text-[var(--portal-blue-gray)]" />
          <h2 className="mt-5 font-serif text-2xl font-light text-[var(--portal-navy)]">
            No transaction workflows yet
          </h2>
          <p className="mx-auto mt-3 max-w-md text-sm font-light text-black/55">
            A workflow instance is created when an offer is accepted and the
            transaction moves into contract preparation.
          </p>
        </section>
      ) : (
        <div className="grid gap-4 xl:grid-cols-2">
          {summaries.map((s) => (
            <WorkflowCard key={s.instanceId} summary={s} />
          ))}
        </div>
      )}
    </div>
  )
}
