"use client"

import { useCallback, useState } from "react"
import type { ReactNode } from "react"

import { loadWorkflowInstanceDetail } from "@/app/portal/workflow-diagnostics-actions"
import type {
  CommandRow,
  CorrelationRow,
  InstanceDetail,
  TaskRow,
  TokenRow,
  WorkflowAnomaly,
  WorkflowDiagnosticsSnapshot,
  WorkflowInstanceRow,
} from "@/workflow_app/diagnostics"

// ---------------------------------------------------------------------------
// Workflow Engine diagnostics section (CRM-14N). Rendered inside the existing
// IT support page (/portal/system-health). Read-only — no engine changes, no
// XML changes, no workflow mutation, no reset control.
// ---------------------------------------------------------------------------

function MetricCard({
  label,
  value,
  detail,
  emphasize,
}: {
  label: string
  value: string
  detail: string
  emphasize?: boolean
}) {
  return (
    <div className="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
        {label}
      </div>

      <div
        className={`mt-4 font-serif text-3xl font-light ${
          emphasize ? "text-red-700" : "text-[var(--portal-navy)]"
        }`}
      >
        {value}
      </div>

      <div className="mt-2 text-xs font-light text-black/40">{detail}</div>
    </div>
  )
}

function SeverityPill({ severity }: { severity: WorkflowAnomaly["severity"] }) {
  const tone =
    severity === "critical"
      ? "bg-red-50 text-red-700 border-red-200"
      : severity === "warning"
        ? "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)] border-[var(--portal-border)]"
        : "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)] border-[var(--portal-border)]"

  return (
    <span
      className={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-[10px] font-medium uppercase tracking-[0.14em] ${tone}`}
    >
      {severity}
    </span>
  )
}

function StatusPill({
  status,
  outcome,
}: {
  status: string
  outcome: string | null
}) {
  let tone = "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)] border-[var(--portal-border)]"
  if (status === "completed") {
    tone = "bg-emerald-50 text-emerald-700 border-emerald-200"
  } else if (status === "error" || outcome === "failed" || outcome === "conflict") {
    tone = "bg-red-50 text-red-700 border-red-200"
  } else if (outcome === "cancelled" || status === "suspended") {
    tone = "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)] border-[var(--portal-border)]"
  }

  const label = outcome && status !== "active" ? outcome : status

  return (
    <span
      className={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-[10px] font-medium uppercase tracking-[0.14em] ${tone}`}
    >
      {label}
    </span>
  )
}

function formatInstant(value: string | null): string {
  if (!value) return "—"
  const d = new Date(value)
  if (Number.isNaN(d.getTime())) return value
  return d.toLocaleString()
}

function Detail({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div>
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-black/35">
        {label}
      </div>
      <div className="mt-2 break-words text-sm font-light leading-6 text-black/70">
        {value}
      </div>
    </div>
  )
}

function SectionTitle({
  title,
  hint,
}: {
  title: string
  hint?: string
}) {
  return (
    <div>
      <h3 className="font-serif text-lg font-light">{title}</h3>
      {hint ? <p className="mt-1 text-xs font-light text-black/40">{hint}</p> : null}
    </div>
  )
}

function TokenTree({
  tokens,
  nodeLabels,
}: {
  tokens: TokenRow[]
  nodeLabels: Record<string, string>
}) {
  const children = new Map<string, TokenRow[]>()
  const roots: TokenRow[] = []
  for (const t of tokens) {
    if (t.parentTokenId) {
      const list = children.get(t.parentTokenId) ?? []
      list.push(t)
      children.set(t.parentTokenId, list)
    } else {
      roots.push(t)
    }
  }

  const render = (token: TokenRow, depth: number): ReactNode => {
    const kids = children.get(token.id) ?? []
    return (
      <div key={token.id}>
        <div
          className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-[var(--portal-border)]/60 py-2"
          style={{ paddingLeft: depth * 18 }}
        >
          <span className="font-mono text-[11px] text-black/45">{token.id}</span>
          <span className="text-sm font-light text-black/70">
            {nodeLabels[token.nodeId] ?? token.nodeId}
          </span>
          <span className="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
            {token.status}
          </span>
          {token.outcome ? (
            <span className="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
              {token.outcome}
            </span>
          ) : null}
          <span className="text-[10px] font-light text-black/35">
            {token.required ? "required" : "optional"}
          </span>
        </div>
        {kids.map((k) => render(k, depth + 1))}
      </div>
    )
  }

  if (roots.length === 0) {
    return <p className="text-sm font-light text-black/40">No tokens.</p>
  }

  return <div>{roots.map((r) => render(r, 0))}</div>
}

function FactsBlock({ variables }: { variables: Record<string, unknown> | null }) {
  if (!variables || Object.keys(variables).length === 0) {
    return <p className="text-sm font-light text-black/40">No facts recorded.</p>
  }
  return (
    <pre className="overflow-x-auto rounded-sm bg-[var(--portal-blue-pale)]/50 p-4 font-mono text-[11px] leading-5 text-black/70">
      {JSON.stringify(variables, null, 2)}
    </pre>
  )
}

function InstanceDetailView({
  detail,
  definitionName,
}: {
  detail: InstanceDetail
  definitionName: string | null
}) {
  return (
    <div className="mt-4 space-y-6">
      <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
        <SectionTitle title="Process / Definition" />
        <div className="mt-4 grid gap-6 md:grid-cols-2 xl:grid-cols-4">
          <Detail
            label="Definition"
            value={definitionName ? `${definitionName} (${detail.definitionKey} v${detail.definitionVersion})` : `${detail.definitionKey} v${detail.definitionVersion}`}
          />
          <Detail label="Instance" value={<span className="font-mono text-xs">{detail.instanceId}</span>} />
          <Detail
            label="Status"
            value={<StatusPill status={detail.status} outcome={detail.outcome} />}
          />
          <Detail
            label="Subject"
            value={detail.subjectType ? `${detail.subjectType}:${detail.subjectId ?? "—"}` : "—"}
          />
          <Detail label="Started" value={formatInstant(detail.startedAt)} />
          <Detail label="Ended" value={formatInstant(detail.endedAt)} />
          <Detail
            label="Counts"
            value={`${detail.activeTokenCount} active tokens · ${detail.taskCount} tasks · ${detail.eventCount} events`}
          />
        </div>
      </section>

      <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
        <SectionTitle title="Tokens" hint="Nested token tree (technical id + human label)." />
        <div className="mt-4">
          <TokenTree tokens={detail.tokens} nodeLabels={detail.nodeLabels} />
        </div>
      </section>

      <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
        <SectionTitle title="Engine Tasks" />
        <div className="mt-4 overflow-x-auto">
          {detail.tasks.length === 0 ? (
            <p className="text-sm font-light text-black/40">No engine tasks.</p>
          ) : (
            <table className="w-full min-w-[640px] text-left text-sm font-light">
              <thead>
                <tr className="text-[10px] uppercase tracking-[0.14em] text-black/35">
                  <th className="py-2 pr-4 font-light">Task</th>
                  <th className="py-2 pr-4 font-light">Status</th>
                  <th className="py-2 pr-4 font-light">Token</th>
                  <th className="py-2 pr-4 font-light">Assignee</th>
                  <th className="py-2 font-light">Candidates</th>
                </tr>
              </thead>
              <tbody>
                {detail.tasks.map((t: TaskRow) => (
                  <tr key={t.id} className="border-t border-[var(--portal-border)]/60">
                    <td className="py-2 pr-4">
                      <div className="text-black/70">{t.name}</div>
                      <div className="font-mono text-[11px] text-black/40">{t.id}</div>
                    </td>
                    <td className="py-2 pr-4 text-black/60">{t.status}</td>
                    <td className="py-2 pr-4 font-mono text-[11px] text-black/50">
                      {t.tokenId ?? "—"}
                    </td>
                    <td className="py-2 pr-4 text-black/60">{t.assignee ?? "—"}</td>
                    <td className="py-2 text-black/60">
                      {t.candidates.length > 0 ? t.candidates.join(", ") : "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </section>

      <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
        <SectionTitle title="Canonical Correlations" />
        <div className="mt-4 overflow-x-auto">
          {detail.correlations.length === 0 ? (
            <p className="text-sm font-light text-black/40">No correlations.</p>
          ) : (
            <table className="w-full min-w-[640px] text-left text-sm font-light">
              <thead>
                <tr className="text-[10px] uppercase tracking-[0.14em] text-black/35">
                  <th className="py-2 pr-4 font-light">Engine Task</th>
                  <th className="py-2 pr-4 font-light">Canonical Task</th>
                  <th className="py-2 pr-4 font-light">Title</th>
                  <th className="py-2 font-light">Status</th>
                </tr>
              </thead>
              <tbody>
                {detail.correlations.map((c: CorrelationRow) => (
                  <tr key={c.workflowTaskId} className="border-t border-[var(--portal-border)]/60">
                    <td className="py-2 pr-4 font-mono text-[11px] text-black/50">
                      {c.workflowTaskId}
                    </td>
                    <td className="py-2 pr-4 font-mono text-[11px] text-black/50">
                      {c.applicationTaskId ?? "—"}
                    </td>
                    <td className="py-2 pr-4 text-black/70">
                      {c.applicationTaskTitle ?? "—"}
                    </td>
                    <td className="py-2 text-black/60">{c.applicationTaskStatus ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </section>

      <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
        <SectionTitle title="Jobs / Timers" />
        <div className="mt-4 overflow-x-auto">
          {detail.jobs.length === 0 ? (
            <p className="text-sm font-light text-black/40">No jobs.</p>
          ) : (
            <table className="w-full min-w-[560px] text-left text-sm font-light">
              <thead>
                <tr className="text-[10px] uppercase tracking-[0.14em] text-black/35">
                  <th className="py-2 pr-4 font-light">Job</th>
                  <th className="py-2 pr-4 font-light">Type</th>
                  <th className="py-2 pr-4 font-light">Status</th>
                  <th className="py-2 font-light">Due</th>
                </tr>
              </thead>
              <tbody>
                {detail.jobs.map((j) => (
                  <tr key={j.id} className="border-t border-[var(--portal-border)]/60">
                    <td className="py-2 pr-4 font-mono text-[11px] text-black/50">{j.id}</td>
                    <td className="py-2 pr-4 text-black/70">{j.type}</td>
                    <td className="py-2 pr-4 text-black/60">{j.status}</td>
                    <td className="py-2 text-black/60">{formatInstant(j.dueAt)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </section>

      <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
        <SectionTitle title="Facts" hint="Process variables captured at runtime." />
        <div className="mt-4">
          <FactsBlock variables={detail.variables} />
        </div>
      </section>

      <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
        <SectionTitle title="Command Receipts" hint="Engine commands and their application-side receipt outcome." />
        <div className="mt-4 overflow-x-auto">
          {detail.commands.length === 0 ? (
            <p className="text-sm font-light text-black/40">No commands executed.</p>
          ) : (
            <table className="w-full min-w-[720px] text-left text-sm font-light">
              <thead>
                <tr className="text-[10px] uppercase tracking-[0.14em] text-black/35">
                  <th className="py-2 pr-4 font-light">Command</th>
                  <th className="py-2 pr-4 font-light">Node</th>
                  <th className="py-2 pr-4 font-light">Outcome</th>
                  <th className="py-2 pr-4 font-light">Receipt</th>
                  <th className="py-2 font-light">Message</th>
                </tr>
              </thead>
              <tbody>
                {detail.commands.map((c: CommandRow) => (
                  <tr key={c.commandId} className="border-t border-[var(--portal-border)]/60">
                    <td className="py-2 pr-4">
                      <div className="text-black/70">{c.commandType}</div>
                      <div className="font-mono text-[11px] text-black/40">{c.commandId}</div>
                    </td>
                    <td className="py-2 pr-4 text-black/60">{c.nodeId}</td>
                    <td className="py-2 pr-4 text-black/60">{c.outcome}</td>
                    <td className="py-2 pr-4">
                      {c.receiptOutcome ? (
                        <span
                          className={`inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.12em] ${
                            c.receiptOutcome === "pending"
                              ? "bg-red-50 text-red-700 border-red-200"
                              : "bg-emerald-50 text-emerald-700 border-emerald-200"
                          }`}
                        >
                          {c.receiptOutcome}
                        </span>
                      ) : (
                        <span className="text-black/40">—</span>
                      )}
                    </td>
                    <td className="py-2 text-black/60">{c.message ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </section>

      <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
        <SectionTitle title="Events" hint="Engine process events (most recent first)." />
        <div className="mt-4 overflow-x-auto">
          {detail.events.length === 0 ? (
            <p className="text-sm font-light text-black/40">No events.</p>
          ) : (
            <table className="w-full min-w-[640px] text-left text-sm font-light">
              <thead>
                <tr className="text-[10px] uppercase tracking-[0.14em] text-black/35">
                  <th className="py-2 pr-4 font-light">Event</th>
                  <th className="py-2 pr-4 font-light">Node</th>
                  <th className="py-2 font-light">Actor</th>
                </tr>
              </thead>
              <tbody>
                {detail.events.map((e) => (
                  <tr key={e.id} className="border-t border-[var(--portal-border)]/60">
                    <td className="py-2 pr-4 text-black/70">{e.eventType}</td>
                    <td className="py-2 pr-4 text-black/60">{e.nodeId ?? "—"}</td>
                    <td className="py-2 text-black/60">{e.actor ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </section>
    </div>
  )
}

export function WorkflowDiagnostics({
  diagnostics,
}: {
  diagnostics: WorkflowDiagnosticsSnapshot
}) {
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [detail, setDetail] = useState<InstanceDetail | null>(null)
  const [loadingId, setLoadingId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const definitionNameByKeyVersion = new Map<string, string>()
  for (const d of diagnostics.definitions) {
    definitionNameByKeyVersion.set(`${d.key}@${d.version}`, d.name)
  }

  const toggleInstance = useCallback(
    async (id: string) => {
      if (selectedId === id) {
        setSelectedId(null)
        setDetail(null)
        setError(null)
        return
      }
      setSelectedId(id)
      setDetail(null)
      setError(null)
      setLoadingId(id)
      try {
        const result = await loadWorkflowInstanceDetail(id)
        setDetail(result)
        if (!result) setError(`No detail found for instance ${id}.`)
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      } finally {
        setLoadingId(null)
      }
    },
    [selectedId]
  )

  const { summary } = diagnostics

  return (
    <div className="mt-10">
      <div className="mb-6">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Workflow
        </p>
        <h2 className="mt-3 font-serif text-3xl font-light leading-[1.1]">
          Workflow Engine
        </h2>
        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Read-only diagnostics for the residential transaction engine — deployed
          definitions, instance lifecycle, correlations, jobs, receipts and
          support anomaly flags.
        </p>
      </div>

      {!diagnostics.configured ? (
        <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-16 text-center">
          <p className="text-sm font-light text-black/50">
            Workflow engine tables are not available in this environment.
          </p>
        </section>
      ) : (
        <>
          <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            <MetricCard
              label="Definitions"
              value={String(summary.definitionCount)}
              detail="Deployed process definitions"
            />
            <MetricCard
              label="Instances"
              value={String(summary.instanceTotal)}
              detail={`${summary.instanceActive} active · ${summary.instanceCompleted} completed · ${summary.instanceFailed} failed · ${summary.instanceOther} other`}
            />
            <MetricCard
              label="Ready Engine Tasks"
              value={String(summary.readyEngineTasks)}
              detail="ready / reserved / in progress"
            />
            <MetricCard
              label="Correlated Open Tasks"
              value={String(summary.correlatedOpenCanonicalTasks)}
              detail="Open canonical tasks with a correlation"
            />
            <MetricCard
              label="Pending Jobs"
              value={String(summary.pendingJobs)}
              detail="pending / locked jobs"
            />
            <MetricCard
              label="Pending Receipts"
              value={String(summary.pendingReceipts)}
              detail="Stuck command receipts"
            />
            <MetricCard
              label="Anomalies"
              value={String(summary.anomalyCount)}
              detail="Support flags to review"
              emphasize={summary.anomalyCount > 0}
            />
          </section>

          <section className="mt-6 rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
            <h3 className="font-serif text-xl font-light">Anomalies</h3>
            <p className="mt-1 text-xs font-light text-black/40">
              Deterministic support flags derived from engine state and
              correlation invariants.
            </p>

            <div className="mt-4 space-y-2">
              {diagnostics.anomalies.length === 0 ? (
                <p className="text-sm font-light text-black/40">
                  No anomalies detected.
                </p>
              ) : (
                diagnostics.anomalies.map((a: WorkflowAnomaly, idx: number) => (
                  <div
                    key={`${a.kind}-${idx}`}
                    className="flex flex-wrap items-start gap-x-3 gap-y-2 rounded-sm border border-[var(--portal-border)]/70 bg-white p-3"
                  >
                    <SeverityPill severity={a.severity} />
                    <div className="min-w-0 flex-1 text-sm font-light leading-6 text-black/70">
                      {a.message}
                    </div>
                    {a.instanceId ? (
                      <span className="font-mono text-[11px] text-black/40">
                        {a.instanceId}
                      </span>
                    ) : null}
                  </div>
                ))
              )}
            </div>
          </section>

          <section className="mt-6 rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
            <div className="flex items-center justify-between">
              <h3 className="font-serif text-xl font-light">Instances</h3>
              <span className="text-xs font-light text-black/40">
                {diagnostics.instances.length} total
              </span>
            </div>

            <div className="mt-4 space-y-2">
              {diagnostics.instances.length === 0 ? (
                <p className="text-sm font-light text-black/40">
                  No workflow instances recorded.
                </p>
              ) : (
                diagnostics.instances.map((inst: WorkflowInstanceRow) => {
                  const selected = selectedId === inst.instanceId
                  return (
                    <div
                      key={inst.instanceId}
                      className="rounded-sm border border-[var(--portal-border)] bg-white"
                    >
                      <button
                        type="button"
                        onClick={() => toggleInstance(inst.instanceId)}
                        className="flex min-h-[48px] w-full flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3 text-left"
                      >
                        <div className="min-w-0 flex-1">
                          <div className="truncate text-sm font-light text-black/80">
                            {inst.propertyName ??
                              (inst.subjectType
                                ? `${inst.subjectType}:${inst.subjectId ?? "—"}`
                                : "—")}
                          </div>
                          <div className="font-mono text-[11px] text-black/40">
                            {inst.definitionKey} v{inst.definitionVersion}
                          </div>
                        </div>

                        <StatusPill status={inst.status} outcome={inst.outcome} />

                        <div className="hidden text-right sm:block">
                          <div className="text-[10px] font-light uppercase tracking-[0.14em] text-black/35">
                            Started
                          </div>
                          <div className="text-xs font-light text-black/60">
                            {formatInstant(inst.startedAt)}
                          </div>
                        </div>

                        <div className="hidden text-right md:block">
                          <div className="text-[10px] font-light uppercase tracking-[0.14em] text-black/35">
                            Tokens
                          </div>
                          <div className="text-xs font-light text-black/60">
                            {inst.activeTokenCount} active
                          </div>
                        </div>

                        <span
                          className={`text-xs text-black/40 transition-transform ${
                            selected ? "rotate-90" : ""
                          }`}
                        >
                          ›
                        </span>
                      </button>

                      {selected ? (
                        <div className="border-t border-[var(--portal-border)] px-4 pb-6">
                          {loadingId === inst.instanceId ? (
                            <p className="py-6 text-sm font-light text-black/40">
                              Loading technical detail…
                            </p>
                          ) : error ? (
                            <p className="py-6 text-sm font-light text-red-700">
                              {error}
                            </p>
                          ) : detail ? (
                            <InstanceDetailView
                              detail={detail}
                              definitionName={
                                definitionNameByKeyVersion.get(
                                  `${inst.definitionKey}@${inst.definitionVersion}`
                                ) ?? null
                              }
                            />
                          ) : (
                            <p className="py-6 text-sm font-light text-black/40">
                              No detail available.
                            </p>
                          )}
                        </div>
                      ) : null}
                    </div>
                  )
                })
              )}
            </div>
          </section>
        </>
      )}
    </div>
  )
}
