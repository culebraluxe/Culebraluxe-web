"use client"

import { useEffect, useState, useTransition } from "react"
import { ChevronDown } from "lucide-react"

import { updateWbsItemAction } from "@/app/portal/wbs/actions"
import type { ProjectWorkNode, ProjectWorkStatus } from "@/ui/projects"
import { PROJECTS_PRIMITIVES } from "@/ui/projects"

const STATUS_LABEL: Record<ProjectWorkStatus, string> = {
  complete: "Complete",
  waiting: "Waiting",
  "in-progress": "In progress",
  "not-started": "Not started",
  blocked: "Blocked",
  dismissed: "Dismissed",
}

function toPersistedStatus(status: ProjectWorkStatus): "open" | "doing" | "done" | "dismissed" {
  if (status === "complete") return "done"
  if (status === "dismissed") return "dismissed"
  if (status === "in-progress") return "doing"
  return "open"
}

function shortDate(value: string, fallback?: string): string {
  if (!value) return fallback ?? "No due date"
  const parsed = new Date(`${value}T12:00:00`)
  if (Number.isNaN(parsed.getTime())) return value
  return parsed.toLocaleDateString("en-US", { month: "short", day: "numeric" })
}

export function SelectedWorkPanel({
  node,
  onSaved,
}: {
  node: ProjectWorkNode | null
  onSaved?: () => void
}) {
  const [collapsed, setCollapsed] = useState(false)
  const [status, setStatus] = useState<ProjectWorkStatus>(node?.status ?? "not-started")
  const [dueAt, setDueAt] = useState(node?.dueAt?.slice(0, 10) ?? "")
  const [owner, setOwner] = useState(node?.owner ?? "")
  const [notes, setNotes] = useState(node?.note ?? "")
  const [saveError, setSaveError] = useState<string | null>(null)
  const [saving, startSaving] = useTransition()

  useEffect(() => {
    setStatus(node?.status ?? "not-started")
    setDueAt(node?.dueAt?.slice(0, 10) ?? "")
    setOwner(node?.owner ?? "")
    setNotes(node?.note ?? "")
    setSaveError(null)
    if (node) setCollapsed(false)
  }, [node])

  const save = () => {
    if (!node) return
    setSaveError(null)
    startSaving(async () => {
      const result = await updateWbsItemAction({
        id: node.id,
        status: toPersistedStatus(status),
        dueAt: dueAt ? new Date(`${dueAt}T12:00:00`).toISOString() : null,
        owner: owner.trim() || null,
        notes,
      })
      if (!result.ok) {
        setSaveError(result.message)
        return
      }
      onSaved?.()
    })
  }

  const launcherLayout = (
    <style jsx global>{`
      /* Projects layout trial: reclaim the standalone New Project row without
         touching the navigator or the MVI/service path. The existing launcher
         stays the same control; it is simply taken out of flow and parked at the
         far-right edge of the project-view row. */
      div:has(> .projects-workspace-grid) > div.flex.shrink-0.justify-end {
        position: absolute;
        z-index: 10;
        top: 2.75rem;
        right: 0.75rem;
      }

      .projects-pane-canvas nav[aria-label="Project workspace views"] {
        width: max-content;
        min-width: 0;
      }

      .projects-pane-canvas div:has(> nav[aria-label="Project workspace views"]) {
        padding-right: 7.75rem;
      }

      div:has(> .projects-workspace-grid) > div.absolute.right-0.top-10.z-20 {
        top: 5.5rem;
      }
    `}</style>
  )

  if (!node) {
    return (
      <>
        {launcherLayout}
        <section className="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
          <div className="flex min-h-10 w-full items-center gap-3 px-3 py-2 text-left">
            <span className="shrink-0 text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
              Selected work
            </span>
            <span className="min-w-0 flex-1 truncate text-[13px] font-light text-black/45">
              Select a real work item to edit it.
            </span>
          </div>
        </section>
      </>
    )
  }

  const header = (
    <button
      type="button"
      onClick={() => setCollapsed((value) => !value)}
      className="flex min-h-10 w-full items-center gap-3 px-3 py-2 text-left"
      aria-expanded={!collapsed}
    >
      <span className="shrink-0 text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
        Selected work
      </span>
      <span className="min-w-0 flex-1 truncate text-[15px] font-medium text-[var(--portal-navy)]">{node.title}</span>
      <span className="hidden shrink-0 text-[12px] font-light text-[var(--portal-blue-gray)] sm:inline">
        {STATUS_LABEL[status]} · {shortDate(dueAt, node.dueLabel)} · {owner.trim() || "Unassigned"}
      </span>
      <span className="flex shrink-0 items-center gap-1 text-[11px] font-medium text-[var(--portal-blue-gray)]">
        {collapsed ? "Expand" : "Collapse"}
        <ChevronDown className={`h-4 w-4 transition ${collapsed ? "" : "rotate-180"}`} aria-hidden />
      </span>
    </button>
  )

  if (collapsed) {
    return (
      <>
        {launcherLayout}
        <section className="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
          {header}
        </section>
      </>
    )
  }

  return (
    <>
      {launcherLayout}
      <section className="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
        {header}
        <div className="grid grid-cols-2 gap-2 border-t border-[var(--portal-panel-border)] px-3 pb-3 pt-2 md:grid-cols-3 xl:grid-cols-[minmax(120px,0.8fr)_145px_minmax(150px,0.9fr)_minmax(260px,2fr)_auto_auto] xl:items-end">
          <label className="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
            Status
            <select
              value={status}
              onChange={(event) => setStatus(event.target.value as ProjectWorkStatus)}
              className={`mt-1 h-9 ${PROJECTS_PRIMITIVES.input()}`}
            >
              <option value="not-started">Not started</option>
              <option value="in-progress">In progress</option>
              <option value="complete">Complete</option>
              <option value="dismissed">Dismissed</option>
            </select>
          </label>

          <label className="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
            Due
            <input
              type="date"
              value={dueAt}
              onChange={(event) => setDueAt(event.target.value)}
              className={`mt-1 h-9 ${PROJECTS_PRIMITIVES.input()}`}
            />
          </label>

          <label className="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
            Assignee
            <input
              value={owner}
              onChange={(event) => setOwner(event.target.value)}
              placeholder="Unassigned"
              className={`mt-1 h-9 ${PROJECTS_PRIMITIVES.input()}`}
            />
          </label>

          <label className="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
            Notes
            <textarea
              value={notes}
              onChange={(event) => setNotes(event.target.value)}
              placeholder="Add a note…"
              rows={2}
              className={`mt-1 min-h-[3.5rem] resize-none leading-snug ${PROJECTS_PRIMITIVES.input()}`}
            />
          </label>

          <label className="flex h-9 items-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-3 text-[12px] font-medium text-[var(--portal-navy)] xl:mb-0">
            <input
              type="checkbox"
              checked={status === "complete"}
              onChange={(event) => setStatus(event.target.checked ? "complete" : "in-progress")}
              className="h-4 w-4 accent-[var(--portal-success)]"
            />
            Complete
          </label>

          <button
            type="button"
            disabled={saving}
            onClick={save}
            className="h-9 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[12px] font-medium text-white shadow-sm transition hover:opacity-90 disabled:opacity-50"
          >
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
        {saveError ? <p className="px-3 pb-2 text-[11px] text-[var(--portal-archive)]">{saveError}</p> : null}
      </section>
    </>
  )
}
