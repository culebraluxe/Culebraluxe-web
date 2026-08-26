"use client"

import { useState } from "react"
import Link from "next/link"
import { PageHeader } from "@/components/portal/page-header"
import { Panel } from "@/components/portal/panel"
import { SEVERITY_STYLE } from "@/lib/issue-types"
import type { IssuesPageResult, IssueQueueRow } from "@/db/issues"
import { resolveIssueAction } from "@/app/portal/issues/actions"

const PAGE_SIZE = 50

function SeverityMark({
  severity,
  compact = false,
}: {
  severity: IssueQueueRow["severity"]
  compact?: boolean
}) {
  const s = SEVERITY_STYLE[severity]
  if (compact) {
    return (
      <span
        className={`inline-flex h-2.5 w-2.5 shrink-0 rounded-full ${s.dot}`}
        aria-label={severity}
      />
    )
  }
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.12em] ${s.badge}`}
    >
      <span className={`h-1.5 w-1.5 rounded-full ${s.dot}`} />
      {severity}
    </span>
  )
}

function relatedLine(row: IssueQueueRow): string {
  const parts: string[] = []
  if (row.propertyName) parts.push(row.propertyName)
  if (row.clientName) parts.push(row.clientName)
  if (parts.length === 0 && row.domainType === "task" && row.taskTitle) {
    parts.push(row.taskTitle)
  }
  return parts.join(" · ")
}

function factsFor(row: IssueQueueRow): { label: string; value: string }[] {
  const facts: { label: string; value: string }[] = []
  if (row.propertyName) facts.push({ label: "Property", value: row.propertyName })
  if (row.clientName) facts.push({ label: "Client", value: row.clientName })
  if (row.closingDate)
    facts.push({ label: "Closing date", value: row.closingDate.slice(0, 10) })
  if (row.dealStage)
    facts.push({ label: "Deal stage", value: row.dealStage.replace(/_/g, " ") })
  if (row.taskTitle) facts.push({ label: "Task", value: row.taskTitle })
  if (row.taskDueAt) facts.push({ label: "Task due", value: row.taskDueAt })
  if (row.detectedAt)
    facts.push({ label: "Detected time", value: new Date(row.detectedAt).toLocaleString() })
  return facts
}

function IssueDetail({
  row,
  onResolved,
}: {
  row: IssueQueueRow
  onResolved: () => void
}) {
  const [busy, setBusy] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)

  async function markResolved() {
    setBusy(true)
    try {
      const res = await resolveIssueAction(row.id)
      setNotice(res.message)
      if (res.ok) onResolved()
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <Panel
        compact
        eyebrow="Issue Summary"
        heading={row.title}
        action={<SeverityMark severity={row.severity} />}
      >
        <p className="text-sm font-light leading-6 text-black/60">
          {row.detail || "No additional detail."}
        </p>
      </Panel>

      <Panel compact heading="Relevant Facts" divider flush>
        <div className="grid divide-y divide-[var(--portal-panel-border)]">
          {factsFor(row).map((fact) => (
            <div
              key={fact.label}
              className="flex items-baseline justify-between gap-3 px-4 py-2"
            >
              <span className="text-[10px] font-medium uppercase tracking-[0.14em] text-black/40">
                {fact.label}
              </span>
              <span className="text-right text-sm font-light text-[var(--portal-navy)]">
                {fact.value}
              </span>
            </div>
          ))}
        </div>
      </Panel>

      <Panel compact heading="Runbook" eyebrow={row.typeLabel} divider>
        <ol className="space-y-3">
          {row.runbook.steps.map((step, i) => (
            <li key={i} className="flex gap-3">
              <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] text-[10px] font-medium text-[var(--portal-navy-soft)]">
                {i + 1}
              </span>
              <div>
                <p className="text-sm font-medium text-[var(--portal-navy)]">
                  {step.title}
                </p>
                <p className="mt-0.5 text-sm font-light leading-6 text-black/55">
                  {step.body}
                </p>
              </div>
            </li>
          ))}
        </ol>
      </Panel>

      <Panel compact heading="Actions" divider>
        <div className="flex flex-wrap items-center gap-2">
          {row.relatedDealId ? (
            <Link
              href={`/portal/deals/${row.relatedDealId}`}
              className="inline-flex h-10 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)]"
            >
              Open Deal
            </Link>
          ) : null}
          <button
            type="button"
            onClick={markResolved}
            disabled={busy}
            className="inline-flex h-10 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy)] transition hover:bg-white disabled:opacity-50"
          >
            {busy ? "Resolving…" : "Mark Resolved"}
          </button>
        </div>
        {notice ? (
          <p className="mt-2 text-xs font-light text-black/45">{notice}</p>
        ) : null}
      </Panel>
    </div>
  )
}
export function IssuesQueue({
  initialPage,
}: {
  initialPage: IssuesPageResult
}) {
  const [rows, setRows] = useState(initialPage.rows)
  const [total, setTotal] = useState(initialPage.total)
  const [page, setPage] = useState(initialPage.page)
  const [selectedId, setSelectedId] = useState<string | null>(
    initialPage.rows[0]?.id ?? null,
  )
  const [loading, setLoading] = useState(false)

  async function load(target: number) {
    setLoading(true)
    try {
      const res = await fetch(
        `/api/portal/issues?scope=${initialPage.scope}&state=${initialPage.state}&page=${target}&pageSize=${PAGE_SIZE}`,
      )
      const data: IssuesPageResult = await res.json()
      const next = data.rows ?? []
      setRows(next)
      setTotal(data.total ?? 0)
      setPage(data.page ?? target)
      setSelectedId((prev) =>
        prev && next.some((r) => r.id === prev) ? prev : next[0]?.id ?? null,
      )
    } finally {
      setLoading(false)
    }
  }

  function handleResolved(id: string) {
    const next = rows.filter((r) => r.id !== id)
    setRows(next)
    setTotal((t) => Math.max(0, t - 1))
    if (selectedId === id) setSelectedId(next[0]?.id ?? null)
  }

  const selected = rows.find((r) => r.id === selectedId) ?? rows[0] ?? null
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))

  return (
    <div>
      <PageHeader compact eyebrow="OPPS" title="Issue Queue">
        <span className="text-xs font-light text-black/40">
          {total} open {total === 1 ? "issue" : "issues"}
        </span>
      </PageHeader>

      {rows.length === 0 ? (
        <Panel compact heading="All clear">
          <p className="text-sm font-light leading-6 text-black/45">
            No operational issues require attention.
          </p>
        </Panel>
      ) : (
        <div className="grid gap-3 lg:grid-cols-[1.15fr_1fr] lg:gap-4">
          {/* LEFT — dense issue queue (dominant scanning surface) */}
          <section className="portal-glass-panel flex flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div className="flex flex-col">
              {rows.map((row) => {
                const s = SEVERITY_STYLE[row.severity]
                const active = selected?.id === row.id
                return (
                  <button
                    key={row.id}
                    type="button"
                    onClick={() => setSelectedId(row.id)}
                    className={`flex items-start gap-3 border-b border-[var(--portal-panel-border)] px-4 py-3 text-left transition last:border-b-0 ${
                      active ? "bg-white/80" : "hover:bg-white/50"
                    }`}
                  >
                    <span
                      className={`mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full ${s.dot}`}
                    />
                    <span className="min-w-0 flex-1">
                      <span className="truncate text-sm font-medium text-[var(--portal-navy)]">
                        {row.title}
                      </span>
                      <span className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[11px] font-light text-black/45">
                        <span
                          className={`uppercase tracking-[0.08em] ${s.label}`}
                        >
                          {row.typeLabel}
                        </span>
                        {relatedLine(row) ? (
                          <span>· {relatedLine(row)}</span>
                        ) : null}
                      </span>
                    </span>
                    <span className="flex shrink-0 flex-col items-end gap-1">
                      <span
                        className={`text-[10px] font-medium uppercase tracking-[0.1em] ${s.label}`}
                      >
                        {row.severity}
                      </span>
                      <span className="text-[10px] font-light uppercase tracking-[0.1em] text-black/35">
                        {row.ageLabel}
                      </span>
                    </span>
                  </button>
                )
              })}
            </div>
            {totalPages > 1 ? (
              <div className="flex items-center justify-between border-t border-[var(--portal-panel-border)] px-4 py-2">
                <button
                  type="button"
                  disabled={page <= 1 || loading}
                  onClick={() => load(page - 1)}
                  className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)] disabled:opacity-40"
                >
                  Prev
                </button>
                <span className="text-[10px] font-light uppercase tracking-[0.12em] text-black/40">
                  Page {page} of {totalPages}
                </span>
                <button
                  type="button"
                  disabled={page >= totalPages || loading}
                  onClick={() => load(page + 1)}
                  className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)] disabled:opacity-40"
                >
                  Next
                </button>
              </div>
            ) : null}
          </section>

          {/* RIGHT — selected issue resolution workspace */}
          {selected ? (
            <IssueDetail
              key={selected.id}
              row={selected}
              onResolved={() => handleResolved(selected.id)}
            />
          ) : null}
        </div>
      )}
    </div>
  )
}

