import Link from "next/link"
import { GitBranch, AlertCircle } from "lucide-react"

import type { WorkflowSummary } from "@/workflow_app/read-service"

export function WorkflowDashboardCard({
  summaries,
}: {
  summaries: WorkflowSummary[]
}) {
  const active = summaries.filter((s) => s.outcome === null)
  const blocked = active.filter((s) => s.blockerCount > 0)
  const ready = active.filter((s) => s.currentNodes.includes("ready_to_close"))
  const waitingSme = active.filter((s) =>
    ["inspector", "appraiser", "title", "lender", "attorney"].includes(
      s.responsibleParty ?? "",
    ),
  )

  return (
    <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
      <div className="flex items-center justify-between">
        <h2 className="flex items-center gap-2 font-serif text-xl font-light text-[var(--portal-navy)]">
          <GitBranch className="h-4 w-4 text-[var(--portal-blue-gray)]" />
          Transactions in motion
        </h2>
        <Link
          href="/portal/workflows"
          className="text-xs font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)] hover:text-[var(--portal-navy)]"
        >
          View all
        </Link>
      </div>

      <div className="mt-5 grid grid-cols-2 gap-4 lg:grid-cols-4">
        <Metric label="Active" value={active.length} />
        <Metric label="Ready to close" value={ready.length} />
        <Metric label="Waiting on SME" value={waitingSme.length} />
        <Metric label="Blocked" value={blocked.length} alert={blocked.length > 0} />
      </div>

      {active.length === 0 && (
        <p className="mt-4 text-sm font-light text-black/45">
          No active transaction workflows.
        </p>
      )}
    </section>
  )
}

function Metric({
  label,
  value,
  alert = false,
}: {
  label: string
  value: number
  alert?: boolean
}) {
  return (
    <div className="rounded-[var(--portal-tab-radius)] bg-[var(--portal-blue-pale)]/55 px-4 py-3">
      <div className="flex items-center gap-1.5">
        <span className="font-serif text-2xl font-light text-[var(--portal-navy)]">
          {value}
        </span>
        {alert && <AlertCircle className="h-4 w-4 text-[var(--portal-blue-gray)]" />}
      </div>
      <div className="mt-1 text-[11px] font-light uppercase tracking-[0.14em] text-black/50">
        {label}
      </div>
    </div>
  )
}
