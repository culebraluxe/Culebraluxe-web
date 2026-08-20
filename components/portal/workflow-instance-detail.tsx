import Link from "next/link"
import {
  ArrowLeft,
  CheckCircle2,
  Circle,
  GitBranch,
  AlertCircle,
  Clock,
} from "lucide-react"

import type { WorkflowDetail } from "@/workflow_app/read-service"
import {
  transactionCloseV1Graph,
  TRANSACTION_CLOSE_V1_ORDER,
  TRANSACTION_MILESTONE_REQUIRED,
} from "@/workflow_app/definitions/transaction-close-v1"
import { responsibilityFor } from "@/workflow_app/responsibility"
import { deadlineFor } from "@/workflow_app/deadlines"

function nodeLabel(id: string) {
  const node = transactionCloseV1Graph.nodes[id]
  return node?.name ?? id.replace(/_/g, " ")
}

function statusPill(detail: WorkflowDetail) {
  if (detail.outcome === "cancelled") return { label: "Cancelled", cls: "bg-black/5 text-black/55" }
  if (detail.outcome === "failed" || detail.outcome === "conflict") {
    return { label: detail.outcome, cls: "bg-red-50 text-red-700" }
  }
  if (detail.outcome === "completed") return { label: "Closed", cls: "bg-emerald-50 text-emerald-700" }
  return { label: detail.status, cls: "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]" }
}

export function WorkflowInstanceDetail({ detail }: { detail: WorkflowDetail }) {
  const pill = statusPill(detail)
  const completed = new Set(detail.completedMilestones)
  const active = new Set(detail.activeMilestones)
  const blocked = detail.currentNodes.filter((n) => n.endsWith("_blocker"))

  return (
    <div>
      <Link
        href="/portal/workflows"
        className="mb-6 inline-flex items-center gap-2 text-sm font-light text-black/50 hover:text-[var(--portal-navy)]"
      >
        <ArrowLeft className="h-4 w-4" />
        Workflows
      </Link>

      {/* Header */}
      <header className="flex flex-wrap items-start justify-between gap-4 rounded-sm border border-[var(--portal-border)] bg-white p-6 lg:p-8">
        <div>
          <div className="flex items-center gap-2 text-[10px] font-light uppercase tracking-[0.22em] text-black/45">
            <GitBranch className="h-3.5 w-3.5 text-[#c6a15b]" />
            {detail.workflowName} · v{detail.workflowVersion}
          </div>
          <h1 className="mt-2 font-serif text-3xl font-light text-[var(--portal-navy)]">
            {detail.propertyName ?? "Transaction"}
          </h1>
          <p className="mt-1 text-sm font-light text-black/55">
            {detail.responsibleParty ?? "Unassigned"} · started{" "}
            {new Date(detail.startedAt).toLocaleDateString()}
          </p>
        </div>
        <span className={`rounded-full px-3 py-1 text-xs font-light capitalize ${pill.cls}`}>
          {pill.label}
        </span>
      </header>

      <div className="mt-6 grid gap-6 2xl:grid-cols-[minmax(0,1.2fr)_minmax(320px,0.8fr)]">
        {/* Timeline */}
        <section className="rounded-sm border border-[var(--portal-border)] bg-white p-6 lg:p-8">
          <h2 className="font-serif text-xl font-light text-[var(--portal-navy)]">Progress</h2>
          <ol className="mt-6 space-y-0">
            {TRANSACTION_CLOSE_V1_ORDER.map((id, i) => {
              const isMilestone = TRANSACTION_MILESTONE_REQUIRED[id] !== undefined
              const done = completed.has(id)
              const isActive = active.has(id) || detail.currentNodes.includes(id)
              return (
                <li key={id} className="relative flex gap-4 pb-6 last:pb-0">
                  {i < TRANSACTION_CLOSE_V1_ORDER.length - 1 && (
                    <span className="absolute left-[11px] top-6 h-full w-px bg-[var(--portal-border)]" />
                  )}
                  {done ? (
                    <CheckCircle2 className="relative z-10 h-6 w-6 shrink-0 text-emerald-600" />
                  ) : isActive ? (
                    <Clock className="relative z-10 h-6 w-6 shrink-0 text-[#c6a15b]" />
                  ) : (
                    <Circle className="relative z-10 h-6 w-6 shrink-0 text-black/20" />
                  )}
                  <div className="min-w-0">
                    <div className="text-sm font-normal text-[var(--portal-navy)]">
                      {nodeLabel(id)}
                      {isMilestone && !TRANSACTION_MILESTONE_REQUIRED[id] && (
                        <span className="ml-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                          optional
                        </span>
                      )}
                    </div>
                    <div className="text-xs font-light text-black/50">
                      {isMilestone ? deadlineFor(id).label : ""}
                    </div>
                  </div>
                </li>
              )
            })}
          </ol>
        </section>

        {/* Operational work */}
        <section className="space-y-6">
          <div className="rounded-sm border border-[var(--portal-border)] bg-white p-6">
            <h2 className="font-serif text-xl font-light text-[var(--portal-navy)]">Milestones</h2>
            <ul className="mt-4 space-y-2">
              {detail.activeMilestones.map((id) => (
                <li key={id} className="flex items-center justify-between text-sm font-light">
                  <span className="text-[var(--portal-navy)]">{nodeLabel(id)}</span>
                  <span className="text-xs text-black/50">{responsibilityFor(id).owner}</span>
                </li>
              ))}
              {detail.activeMilestones.length === 0 && detail.outcome === null && (
                <li className="text-sm font-light text-black/45">No active milestone.</li>
              )}
            </ul>
          </div>

          <div className="rounded-sm border border-[var(--portal-border)] bg-white p-6">
            <h2 className="font-serif text-xl font-light text-[var(--portal-navy)]">Operational</h2>
            <dl className="mt-4 space-y-2 text-sm font-light text-black/60">
              <div className="flex justify-between">
                <dt>Open tasks</dt>
                <dd>{detail.openTaskCount}</dd>
              </div>
              <div className="flex justify-between">
                <dt>Pending timers</dt>
                <dd>{detail.pendingTimerCount}</dd>
              </div>
              <div className="flex justify-between">
                <dt>Blockers</dt>
                <dd className={blocked.length > 0 ? "text-amber-700" : ""}>
                  {blocked.map(nodeLabel).join(", ") || "—"}
                </dd>
              </div>
            </dl>
          </div>

          <div className="rounded-sm border border-[var(--portal-border)] bg-white p-6">
            <h2 className="font-serif text-xl font-light text-[var(--portal-navy)]">Recent activity</h2>
            <ol className="mt-4 space-y-3">
              {detail.events.slice(0, 8).map((e) => (
                <li key={e.id} className="text-sm font-light">
                  <div className="text-[var(--portal-navy)]">{e.eventType}</div>
                  <div className="text-xs text-black/45">
                    {e.nodeId ? `Node: ${nodeLabel(e.nodeId)} · ` : ""}
                    {e.actor ?? ""}
                  </div>
                </li>
              ))}
              {detail.events.length === 0 && (
                <li className="text-sm font-light text-black/45">No events yet.</li>
              )}
            </ol>
          </div>
        </section>
      </div>
    </div>
  )
}
