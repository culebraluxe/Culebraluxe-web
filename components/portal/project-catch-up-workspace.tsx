"use client"

import { useCallback, useEffect, useMemo, useState, useTransition } from "react"
import { useRouter } from "next/navigation"
import { AlertCircle, CheckCircle2, ChevronRight, Circle, Clock3, ListChecks } from "lucide-react"

import { updateWbsItemAction } from "@/app/portal/wbs/actions"
import { SelectedWorkPanel } from "@/components/portal/selected-work-panel"
import { PROJECTS_SCROLL_CLASS } from "@/ui/projects"
import type { ProjectDomainKey, ProjectWorkStatus } from "@/ui/projects"
import {
  projectCatchUp,
  type CatchUpBucket,
  type ProjectCatchUpItem,
} from "@/ui/projects/catchup-projection"

type CatchUpFilter = "all" | "open" | "blocked" | "complete"

const STATUS_LABEL: Record<ProjectWorkStatus, string> = {
  complete: "Complete",
  waiting: "Waiting",
  "in-progress": "In progress",
  "not-started": "Not started",
  blocked: "Blocked",
  dismissed: "Dismissed",
}

function domainLabel(domain: ProjectDomainKey): string {
  return domain.charAt(0).toUpperCase() + domain.slice(1)
}

function StatusIcon({ status }: { status: ProjectWorkStatus }) {
  if (status === "complete") return <CheckCircle2 className="h-3.5 w-3.5 shrink-0 text-[var(--portal-success)]" aria-hidden />
  if (status === "waiting") return <Clock3 className="h-3.5 w-3.5 shrink-0 text-[var(--portal-gold)]" aria-hidden />
  if (status === "blocked") return <AlertCircle className="h-3.5 w-3.5 shrink-0 text-[var(--portal-archive)]" aria-hidden />
  if (status === "in-progress") return <Circle className="h-3.5 w-3.5 shrink-0 text-[var(--portal-blue-gray)]" aria-hidden />
  if (status === "dismissed") return <Circle className="h-3.5 w-3.5 shrink-0 text-black/35" aria-hidden />
  return <Circle className="h-3.5 w-3.5 shrink-0 text-black/25" aria-hidden />
}

export function ProjectCatchUpWorkspace({
  items,
  today,
  onOpenEntry,
  onNewProject,
}: {
  items: readonly ProjectCatchUpItem[]
  today: Date | null
  onOpenEntry: (entry: ProjectCatchUpItem) => void
  onNewProject?: () => void
}) {
  const router = useRouter()
  const [bucket, setBucket] = useState<CatchUpBucket>("today")
  const [statusFilter, setStatusFilter] = useState<CatchUpFilter>("all")
  const [domainFilter, setDomainFilter] = useState<ProjectDomainKey | "all">("all")
  const [selectedEntryKey, setSelectedEntryKey] = useState<string | null>(null)
  const [pendingNodeId, setPendingNodeId] = useState<string | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)
  const [isCompleting, startCompleting] = useTransition()

  const buckets = useMemo(
    () => (today ? projectCatchUp(items, today) : { today: [], unscheduled: [] }),
    [items, today],
  )

  useEffect(() => {
    if (!today) return
    if (buckets.today.length === 0 && buckets.unscheduled.length > 0) setBucket("unscheduled")
  }, [today, buckets.today.length, buckets.unscheduled.length])

  const entries = bucket === "today" ? buckets.today : buckets.unscheduled

  useEffect(() => {
    if (selectedEntryKey && !entries.some((entry) => entry.id === selectedEntryKey)) {
      setSelectedEntryKey(null)
    }
  }, [entries, selectedEntryKey])

  const selectedEntry = useMemo(
    () => items.find((entry) => entry.id === selectedEntryKey) ?? null,
    [items, selectedEntryKey],
  )

  const completed = entries.filter((entry) => entry.node.status === "complete").length
  const blocked = entries.filter((entry) => entry.node.status === "blocked").length
  const remaining = entries.length - completed
  const availableDomains = useMemo(
    () => Array.from(new Set(entries.map((entry) => entry.domain))),
    [entries],
  )
  const statusCounts = useMemo<Record<CatchUpFilter, number>>(() => ({
    all: entries.length,
    open: entries.filter((entry) => entry.node.status !== "complete").length,
    blocked: blocked,
    complete: completed,
  }), [entries, blocked, completed])
  const visibleEntries = useMemo(
    () => entries.filter((entry) => {
      const status = entry.node.status
      const statusMatch = statusFilter === "all"
        ? true
        : statusFilter === "open"
          ? status !== "complete"
          : status === statusFilter
      const domainMatch = domainFilter === "all" || entry.domain === domainFilter
      return statusMatch && domainMatch
    }),
    [entries, statusFilter, domainFilter],
  )

  const dateLabel = today
    ? today.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })
    : "Today"

  const completeEntry = useCallback((entry: ProjectCatchUpItem) => {
    if (entry.node.status === "complete" || entry.node.status === "blocked" || isCompleting) return
    setMutationError(null)
    setPendingNodeId(entry.node.id)
    startCompleting(async () => {
      const result = await updateWbsItemAction({ id: entry.node.id, status: "done" })
      if (!result.ok) setMutationError(result.message)
      else router.refresh()
      setPendingNodeId(null)
    })
  }, [isCompleting, router])

  const filterOptions: Array<{ key: CatchUpFilter; label: string }> = [
    { key: "all", label: "All" },
    { key: "open", label: "Open" },
    { key: "blocked", label: "Blocked" },
    { key: "complete", label: "Done" },
  ]
  const bucketOptions: Array<{ key: CatchUpBucket; label: string; count: number }> = [
    { key: "today", label: "Today", count: buckets.today.length },
    { key: "unscheduled", label: "Unscheduled", count: buckets.unscheduled.length },
  ]

  return (
    <>
      <div className="border-b border-[var(--portal-panel-border)] px-3 py-2">
        <div className="flex min-h-11 items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2.5">
            <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-[var(--portal-navy)] text-[var(--portal-gold)] shadow-sm">
              <ListChecks className="h-5 w-5" strokeWidth={1.7} aria-hidden />
            </span>
            <span className="min-w-0">
              <span className="block text-[14px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)]">Catch-Up</span>
              <span className="block text-[11px] font-light uppercase tracking-[0.08em] text-[var(--portal-blue-gray)]">
                {bucket === "today" ? `Today · ${dateLabel}` : "Real project work without a due date"}
              </span>
            </span>
          </div>
          <div className="flex shrink-0 items-center gap-3">
            <span className="text-[11px] font-light text-[var(--portal-blue-gray)]">
              {entries.length ? `${remaining} remaining · ${completed} complete${blocked ? ` · ${blocked} blocked` : ""}` : bucket === "today" ? "Nothing due today" : "No unscheduled work"}
            </span>
            {onNewProject ? (
              <button
                type="button"
                onClick={onNewProject}
                className="rounded-full bg-[var(--portal-navy)] px-3.5 py-2 text-[12px] font-medium text-white shadow-sm transition hover:opacity-90"
              >
                New Project
              </button>
            ) : null}
          </div>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 border-b border-[var(--portal-panel-border)]/70 px-3 py-2">
        <span className="mr-1 text-[10px] font-medium uppercase tracking-[0.12em] text-black/35">Worklist</span>
        {bucketOptions.map((option) => {
          const selected = bucket === option.key
          return (
            <button
              key={option.key}
              type="button"
              onClick={() => setBucket(option.key)}
              className={`rounded-full px-2.5 py-1 text-[10px] font-medium transition ${selected ? "bg-[var(--portal-gold)]/20 text-[var(--portal-navy)] ring-1 ring-inset ring-[var(--portal-gold)]/35" : "bg-white/35 text-[var(--portal-navy-soft)] hover:bg-white/55"}`}
            >
              {option.label} <span className="ml-1 opacity-65">{option.count}</span>
            </button>
          )
        })}
        <span className="ml-2 text-[10px] font-medium uppercase tracking-[0.12em] text-black/35">Status</span>
        {filterOptions.map((option) => {
          const selected = statusFilter === option.key
          return (
            <button
              key={option.key}
              type="button"
              onClick={() => setStatusFilter(option.key)}
              className={`rounded-full px-2.5 py-1 text-[10px] font-medium transition ${selected ? "bg-[var(--portal-navy)] text-white" : "bg-white/35 text-[var(--portal-navy-soft)] hover:bg-white/55"}`}
            >
              {option.label} <span className="ml-1 opacity-65">{statusCounts[option.key]}</span>
            </button>
          )
        })}
        <span className="ml-auto text-[10px] font-medium uppercase tracking-[0.12em] text-black/35">Area</span>
        <select
          value={domainFilter}
          onChange={(event) => setDomainFilter(event.target.value as ProjectDomainKey | "all")}
          className="h-7 rounded-full border border-[var(--portal-panel-border)] bg-white/50 px-2.5 text-[10px] font-medium text-[var(--portal-navy-soft)] outline-none"
          aria-label="Filter Catch-Up by area"
        >
          <option value="all">All areas</option>
          {availableDomains.map((domain) => <option key={domain} value={domain}>{domainLabel(domain)}</option>)}
        </select>
        {mutationError ? <span className="basis-full text-[11px] text-[var(--portal-archive)]">{mutationError}</span> : null}
      </div>

      <div className="flex min-h-0 flex-1 flex-col px-3 pb-2 pt-2">
        <div className={`min-h-0 flex-1 ${PROJECTS_SCROLL_CLASS} overflow-x-auto rounded-[var(--portal-tab-radius)] border border-white/40 bg-white/20`}>
          <div className="min-w-[900px]">
            <div className="grid grid-cols-[minmax(0,1fr)_100px] border-b border-[var(--portal-panel-border)]/70 px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-black/40">
              <div className="grid grid-cols-[30px_minmax(230px,1.55fr)_minmax(180px,1fr)_140px_minmax(120px,0.75fr)_24px] items-center gap-3">
                <span />
                <span>Task</span>
                <span>Project</span>
                <span>Area</span>
                <span>Assignee</span>
                <span />
              </div>
              <span className="text-right">Action</span>
            </div>
            {!today ? (
              <div className="flex min-h-[220px] items-center justify-center px-6 text-sm font-light text-black/40">Loading project work…</div>
            ) : visibleEntries.length === 0 ? (
              <div className="flex min-h-[220px] items-center justify-center px-6 text-center text-sm font-light text-black/45">
                {bucket === "today" ? "No project tasks are due today." : "No unscheduled project work matches those filters."}
              </div>
            ) : (
              <ul className="divide-y divide-[var(--portal-panel-border)]/70">
                {visibleEntries.map((entry) => {
                  const done = entry.node.status === "complete"
                  const pending = pendingNodeId === entry.node.id
                  const selected = selectedEntryKey === entry.id
                  return (
                    <li key={entry.id} className={`grid grid-cols-[minmax(0,1fr)_100px] items-stretch ${done ? "opacity-60" : ""} ${selected ? "bg-white/45 ring-1 ring-inset ring-[var(--portal-gold)]/35" : ""}`}>
                      <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_32px] items-stretch">
                        <button
                          type="button"
                          onClick={() => setSelectedEntryKey(entry.id)}
                          title="Edit this work item below"
                          aria-pressed={selected}
                          className="grid w-full grid-cols-[30px_minmax(230px,1.55fr)_minmax(180px,1fr)_140px_minmax(120px,0.75fr)] items-center gap-3 px-3 py-3 text-left transition hover:bg-white/30"
                        >
                          <StatusIcon status={entry.node.status} />
                          <span className="min-w-0">
                            <span className={`block truncate text-[14px] font-medium text-[var(--portal-navy)] ${done ? "line-through" : ""}`}>{entry.node.title}</span>
                            <span className="mt-0.5 block text-[10px] font-light uppercase tracking-[0.08em] text-black/40">{STATUS_LABEL[entry.node.status]}</span>
                          </span>
                          <span className="min-w-0 truncate text-[13px] font-light text-[var(--portal-navy)]">{entry.projectTitle}</span>
                          <span className="min-w-0">
                            <span className="block truncate text-[12px] font-medium text-[var(--portal-navy-soft)]">{domainLabel(entry.domain)}</span>
                            <span className="block truncate text-[10px] font-light text-black/40">{entry.contextLabel ?? "Project work"}</span>
                          </span>
                          <span className="truncate text-[12px] font-light text-[var(--portal-blue-gray)]">{entry.node.owner ?? "—"}</span>
                        </button>
                        <button
                          type="button"
                          onClick={() => onOpenEntry(entry)}
                          title="Open this work item in its project"
                          aria-label={`Open ${entry.node.title} in its project`}
                          className="flex items-center justify-center text-black/25 transition hover:bg-white/30 hover:text-[var(--portal-gold-muted)]"
                        >
                          <ChevronRight className="h-4 w-4" aria-hidden />
                        </button>
                      </div>
                      <div className="flex items-center justify-end px-3 py-2">
                        {done ? (
                          <span className="text-[10px] font-medium uppercase tracking-[0.1em] text-[var(--portal-success)]">Done</span>
                        ) : entry.node.status === "blocked" ? (
                          <button type="button" onClick={() => setSelectedEntryKey(entry.id)} className="rounded-full border border-[var(--portal-archive)]/30 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-archive)]">Edit</button>
                        ) : (
                          <button
                            type="button"
                            disabled={isCompleting}
                            onClick={() => completeEntry(entry)}
                            className="rounded-full border border-[var(--portal-success)]/35 bg-white/30 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-success)] transition hover:bg-white/55 disabled:opacity-40"
                          >
                            {pending ? "Saving…" : "Complete"}
                          </button>
                        )}
                      </div>
                    </li>
                  )
                })}
              </ul>
            )}
          </div>
        </div>
      </div>

      <div className="shrink-0 px-3 pb-3">
        <SelectedWorkPanel node={selectedEntry?.node ?? null} onSaved={() => router.refresh()} />
      </div>
    </>
  )
}
