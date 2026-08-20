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
import { resolveResponsibility } from "@/workflow_app/responsibility"
import { deadlineLabelFor } from "@/workflow_app/deadlines"

// Story 128 — this view is fully definition-driven. Node labels, descriptions,
// responsibility hints, optionality, and timeline order all come from the
// deployed definition (authored in XML). There is no hardcoded workflow-state
// mapping or translation table in the Portal layer.

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
  const completed = new Set(detail.completedNodes)
  const active = new Set(detail.currentNodes)
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
        {/* Timeline (definition display-order) */}
        <section className="rounded-sm border border-[var(--portal-border)] bg-white p-6 lg:p-8">
          <h2 className="font-serif text-xl font-light text-[var(--portal-navy)]">Progress</h2>
          <ol className="mt-6 space-y-0">
            {(detail.displayOrder.length > 0
              ? detail.displayOrder
              : detail.currentNodes
            ).map((id, i, list) => {
              const done = completed.has(id)
              const isActive = active.has(id)
              const optional = detail.optionalNodes.includes(id)
              return (
                <li key={id} className="relative flex gap-4 pb-6 last:pb-0">
                  {i < list.length - 1 && (
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
                      {detail.nodeLabels[id] ?? id.replace(/_/g, " ")}
                      {optional && (
                        <span className="ml-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                          optional
                        </span>
                      )}
                    </div>
                    <div className="text-xs font-light text-black/50">
                      {deadlineLabelFor(id) ?? ""}
                    </div>
                    {detail.nodeDescriptions[id] && (
                      <div className="mt-0.5 max-w-md text-xs font-light leading-5 text-black/45">
                        {detail.nodeDescriptions[id]}
                      </div>
                    )}
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
              {detail.activeMilestoneNodeIds.map((id) => (
                <li key={id} className="flex items-center justify-between text-sm font-light">
                  <span className="text-[var(--portal-navy)]">
                    {detail.nodeLabels[id] ?? id.replace(/_/g, " ")}
                  </span>
                  <span className="text-xs text-black/50">
                    {resolveResponsibility(detail.nodeResponsibility[id]).owner}
                  </span>
                </li>
              ))}
              {detail.activeMilestoneNodeIds.length === 0 && detail.outcome === null && (
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
                  {blocked.map((n) => detail.nodeLabels[n] ?? n).join(", ") || "—"}
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
                    {e.nodeId ? `Node: ${detail.nodeLabels[e.nodeId] ?? e.nodeId} · ` : ""}
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
