"use client"

import { useEffect, useState, useTransition } from "react"
import { ChevronDown } from "lucide-react"

import { createAppleCalendarEventAction } from "@/app/portal/projects/calendar-actions"
import {
  queueAppleReminderForWbsAction,
  updateWbsItemAction,
} from "@/app/portal/wbs/actions"
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

type WorkDestination = "task" | "calendar"

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

function prIso(date: string, time: string): string {
  // Puerto Rico is Atlantic Standard Time year-round (UTC-04:00).
  return new Date(`${date}T${time}:00-04:00`).toISOString()
}

function plusThirtyMinutes(iso: string): string {
  return new Date(new Date(iso).getTime() + 30 * 60 * 1000).toISOString()
}

const LOCAL_APPLE_SYNC_URL = "http://127.0.0.1:47831/sync"

/**
 * Fast path only. The server action has ALREADY committed the durable Apple command
 * to Neon before this runs. On this Mac the loopback listener wakes EventKit now;
 * on iPhone/iPad (or if the Mac listener is down) this quietly returns false and the
 * normal scheduled Mac sync drains the exact same durable command later.
 */
async function kickLocalAppleSync(): Promise<boolean> {
  const controller = new AbortController()
  const timeout = window.setTimeout(() => controller.abort(), 1500)
  try {
    const response = await fetch(LOCAL_APPLE_SYNC_URL, {
      method: "POST",
      mode: "cors",
      cache: "no-store",
      signal: controller.signal,
    })
    return response.ok
  } catch {
    return false
  } finally {
    window.clearTimeout(timeout)
  }
}

export function SelectedWorkPanel({
  node,
  onSaved,
}: {
  node: ProjectWorkNode | null
  onSaved?: () => void
}) {
  const [collapsed, setCollapsed] = useState(false)
  const [destination, setDestination] = useState<WorkDestination>("task")
  const [title, setTitle] = useState(node?.title ?? "")
  const [status, setStatus] = useState<ProjectWorkStatus>(node?.status ?? "not-started")
  const [dueAt, setDueAt] = useState(node?.dueAt?.slice(0, 10) ?? "")
  const [startTime, setStartTime] = useState("09:00")
  const [location, setLocation] = useState("")
  const [notes, setNotes] = useState(node?.note ?? "")
  const [alert, setAlert] = useState(true)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [routeStatus, setRouteStatus] = useState<string | null>(null)
  const [saving, startSaving] = useTransition()

  useEffect(() => {
    setDestination("task")
    setTitle(node?.title ?? "")
    setStatus(node?.status ?? "not-started")
    setDueAt(node?.dueAt?.slice(0, 10) ?? "")
    setStartTime("09:00")
    setLocation("")
    setNotes(node?.note ?? "")
    setAlert(true)
    setSaveError(null)
    setRouteStatus(null)
    if (node) setCollapsed(false)
  }, [node?.id])

  const persistCurrent = async () => {
    if (!node) return { ok: false as const, message: "No work item selected." }
    const cleanTitle = title.trim()
    if (!cleanTitle) return { ok: false as const, message: "Title is required." }
    return updateWbsItemAction({
      id: node.id,
      title: cleanTitle,
      status: toPersistedStatus(status),
      // WBS due dates are calendar dates. Noon UTC preserves YYYY-MM-DD across PR offsets.
      dueAt: dueAt ? `${dueAt}T12:00:00.000Z` : null,
      // Current CulebraLuxe operating model: Lisa owns selected work unless an existing owner is set.
      owner: node.owner?.trim() || "Lisa",
      notes,
    })
  }

  const save = () => {
    if (!node) return
    setSaveError(null)
    setRouteStatus(null)

    const cleanTitle = title.trim()
    if (!cleanTitle) {
      setSaveError("Title is required.")
      return
    }

    if (destination === "calendar" && (!dueAt || !startTime)) {
      setSaveError("Calendar date and start time are required.")
      return
    }

    startSaving(async () => {
      const saved = await persistCurrent()
      if (!saved.ok) {
        setSaveError(saved.message)
        return
      }

      if (destination === "calendar") {
        const startAt = prIso(dueAt, startTime)
        const result = await createAppleCalendarEventAction({
          title: cleanTitle,
          startAt,
          endAt: plusThirtyMinutes(startAt),
          location: location.trim() || null,
          notes: notes.trim() || null,
          alert,
        })
        if (!result.ok) {
          setSaveError(result.message)
          return
        }
        setRouteStatus(`Saved · Apple Calendar queued${alert ? " · alert 15 min before" : ""}.`)
        void kickLocalAppleSync().then((kicked) => {
          if (kicked) {
            setRouteStatus(`Saved · syncing to Apple Calendar now${alert ? " · alert 15 min before" : ""}.`)
          }
        })
      } else {
        const result = await queueAppleReminderForWbsAction(node.id, { alert })
        if (!result.ok) {
          setSaveError(result.message)
          return
        }
        setRouteStatus(`Saved · Apple Reminder queued${alert && dueAt ? " · alert 9 AM on due date" : ""}.`)
        void kickLocalAppleSync().then((kicked) => {
          if (kicked) {
            setRouteStatus(`Saved · syncing to Apple Reminders now${alert && dueAt ? " · alert 9 AM on due date" : ""}.`)
          }
        })
      }

      onSaved?.()
    })
  }

  if (!node) {
    return (
      <section className="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
        <div className="flex min-h-10 w-full items-center gap-3 px-3 py-2 text-left">
          <span className="shrink-0 text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
            Selected work
          </span>
          <span className="min-w-0 flex-1 truncate text-[13px] font-light text-black/45">
            Select a work item to edit it.
          </span>
        </div>
      </section>
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
      <span className="min-w-0 flex-1 truncate text-[15px] font-medium text-[var(--portal-navy)]">{title || node.title}</span>
      <span className="hidden shrink-0 text-[12px] font-light text-[var(--portal-blue-gray)] sm:inline">
        {destination === "calendar" ? "Calendar" : STATUS_LABEL[status]} · {shortDate(dueAt, node.dueLabel)}
      </span>
      <span className="flex shrink-0 items-center gap-1 text-[11px] font-medium text-[var(--portal-blue-gray)]">
        {collapsed ? "Expand" : "Collapse"}
        <ChevronDown className={`h-4 w-4 transition ${collapsed ? "" : "rotate-180"}`} aria-hidden />
      </span>
    </button>
  )

  if (collapsed) {
    return (
      <section className="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
        {header}
      </section>
    )
  }

  return (
    <section className="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
      {header}
      <div className={`grid grid-cols-2 gap-2 border-t border-[var(--portal-panel-border)] px-3 pb-3 pt-2 md:grid-cols-3 ${destination === "calendar" ? "xl:grid-cols-[105px_minmax(180px,1.2fr)_135px_105px_minmax(150px,0.9fr)_95px_minmax(220px,1.35fr)_auto]" : "xl:grid-cols-[105px_minmax(180px,1.2fr)_130px_135px_95px_minmax(220px,1.35fr)_auto_auto]"} xl:items-end`}>
        <label className="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
          Type
          <select
            value={destination}
            onChange={(event) => setDestination(event.target.value as WorkDestination)}
            className={`mt-1 h-9 ${PROJECTS_PRIMITIVES.input()}`}
          >
            <option value="task">Task</option>
            <option value="calendar">Calendar</option>
          </select>
        </label>

        <label className="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
          Title
          <input
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            className={`mt-1 h-9 ${PROJECTS_PRIMITIVES.input()}`}
          />
        </label>

        {destination === "task" ? (
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
        ) : null}

        <label className="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
          {destination === "calendar" ? "Date" : "Due"}
          <input
            type="date"
            value={dueAt}
            onChange={(event) => setDueAt(event.target.value)}
            className={`mt-1 h-9 ${PROJECTS_PRIMITIVES.input()}`}
          />
        </label>

        {destination === "calendar" ? (
          <label className="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
            Start
            <input
              type="time"
              value={startTime}
              onChange={(event) => setStartTime(event.target.value)}
              className={`mt-1 h-9 ${PROJECTS_PRIMITIVES.input()}`}
            />
          </label>
        ) : null}

        {destination === "calendar" ? (
          <label className="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
            Location
            <input
              value={location}
              onChange={(event) => setLocation(event.target.value)}
              placeholder="Optional"
              className={`mt-1 h-9 ${PROJECTS_PRIMITIVES.input()}`}
            />
          </label>
        ) : null}

        <label className="flex h-9 items-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-3 text-[12px] font-medium text-[var(--portal-navy)] xl:mb-0">
          <input
            type="checkbox"
            checked={alert}
            onChange={(event) => setAlert(event.target.checked)}
            className="h-4 w-4 accent-[var(--portal-gold)]"
          />
          Alert
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

        {destination === "task" ? (
          <label className="flex h-9 items-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-3 text-[12px] font-medium text-[var(--portal-navy)] xl:mb-0">
            <input
              type="checkbox"
              checked={status === "complete"}
              onChange={(event) => setStatus(event.target.checked ? "complete" : "in-progress")}
              className="h-4 w-4 accent-[var(--portal-success)]"
            />
            Complete
          </label>
        ) : null}

        <button
          type="button"
          disabled={saving}
          onClick={save}
          className="h-9 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[12px] font-medium text-white shadow-sm transition hover:opacity-90 disabled:opacity-50"
        >
          {saving ? "Saving…" : "Save"}
        </button>
      </div>
      {saveError ? <p className="px-3 pb-1 text-[11px] text-[var(--portal-archive)]">{saveError}</p> : null}
      {routeStatus ? <p className="px-3 pb-2 text-[11px] text-[var(--portal-blue-gray)]">{routeStatus}</p> : null}
    </section>
  )
}
