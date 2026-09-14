"use client"

import { useEffect, useMemo, useState, useTransition } from "react"
import { CalendarDays, Clock3, Plus, X } from "lucide-react"

import {
  createAppleCalendarEventAction,
  loadCatchUpScheduleAction,
} from "@/app/portal/projects/calendar-actions"
import {
  calendarTimeLabel,
  type CatchUpCalendarEvent,
} from "@/lib/catchup/calendar-adapter"

const PR_TIME_ZONE = "America/Puerto_Rico"

function puertoRicoDateKey(value: Date | string): string | null {
  const date = value instanceof Date ? value : new Date(value)
  if (Number.isNaN(date.getTime())) return null
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: PR_TIME_ZONE,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(date)
  const year = parts.find((part) => part.type === "year")?.value
  const month = parts.find((part) => part.type === "month")?.value
  const day = parts.find((part) => part.type === "day")?.value
  return year && month && day ? `${year}-${month}-${day}` : null
}

function sourceLabel(source: string): string {
  if (source === "canonical:showing") return "Showing"
  if (source === "apple_calendar") return "Apple Calendar"
  return "Calendar"
}

function prIso(date: string, time: string): string {
  // Puerto Rico is Atlantic Standard Time year-round (UTC-04:00).
  return new Date(`${date}T${time}:00-04:00`).toISOString()
}

export function ProjectCatchUpSchedule({ today }: { today: Date | null }) {
  const [events, setEvents] = useState<CatchUpCalendarEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [creating, setCreating] = useState(false)
  const [title, setTitle] = useState("")
  const [eventDate, setEventDate] = useState("")
  const [startTime, setStartTime] = useState("09:00")
  const [endTime, setEndTime] = useState("09:30")
  const [createStatus, setCreateStatus] = useState<string | null>(null)
  const [queueing, startQueueing] = useTransition()

  useEffect(() => {
    let cancelled = false
    void loadCatchUpScheduleAction().then((result) => {
      if (cancelled) return
      if (result.ok) {
        setEvents(result.events)
        setError(null)
      } else {
        setEvents([])
        setError(result.message)
      }
      setLoading(false)
    })
    return () => { cancelled = true }
  }, [])

  const todayKey = today ? puertoRicoDateKey(today) : null

  useEffect(() => {
    if (todayKey && !eventDate) setEventDate(todayKey)
  }, [todayKey, eventDate])

  const todaysEvents = useMemo(
    () => events
      .filter((event) => todayKey && puertoRicoDateKey(event.startAt) === todayKey)
      .sort((a, b) => {
        if (a.allDay !== b.allDay) return a.allDay ? -1 : 1
        return a.startAt.localeCompare(b.startAt) || a.title.localeCompare(b.title)
      }),
    [events, todayKey],
  )

  const queueEvent = () => {
    const cleanTitle = title.trim()
    if (!cleanTitle || !eventDate || !startTime || !endTime) {
      setCreateStatus("Title, date and time are required.")
      return
    }
    setCreateStatus(null)
    startQueueing(async () => {
      const result = await createAppleCalendarEventAction({
        title: cleanTitle,
        startAt: prIso(eventDate, startTime),
        endAt: prIso(eventDate, endTime),
      })
      if (!result.ok) {
        setCreateStatus(result.message)
        return
      }
      setTitle("")
      setCreateStatus("Queued for Apple Calendar.")
      setCreating(false)
    })
  }

  return (
    <section className="shrink-0 border-b border-[var(--portal-panel-border)]/70 px-3 py-2" aria-label="Today's schedule">
      <div className="flex items-center gap-2">
        <CalendarDays className="h-4 w-4 shrink-0 text-[var(--portal-gold-muted)]" aria-hidden />
        <span className="text-[10px] font-semibold uppercase tracking-[0.12em] text-black/40">Schedule</span>
        <span className="rounded-full bg-white/40 px-2 py-0.5 text-[10px] font-medium text-[var(--portal-navy-soft)]">
          {loading ? "…" : todaysEvents.length}
        </span>
        <span className="text-[10px] font-light text-black/35">Apple Calendar</span>
        <button
          type="button"
          onClick={() => { setCreating((value) => !value); setCreateStatus(null) }}
          className="ml-auto flex h-7 items-center gap-1 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-2.5 text-[11px] font-medium text-[var(--portal-navy)] transition hover:bg-white/70"
        >
          {creating ? <X className="h-3.5 w-3.5" aria-hidden /> : <Plus className="h-3.5 w-3.5" aria-hidden />}
          {creating ? "Cancel" : "Event"}
        </button>
      </div>

      {creating ? (
        <div className="mt-2 flex flex-wrap items-end gap-2 rounded-[var(--portal-tab-radius)] border border-white/45 bg-white/25 p-2">
          <label className="min-w-[220px] flex-1 text-[10px] font-semibold uppercase tracking-[0.08em] text-black/45">
            Event
            <input
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              placeholder="Appointment or meeting"
              className="mt-1 h-8 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/60 px-2.5 text-[12px] font-normal normal-case tracking-normal text-[var(--portal-navy)] outline-none"
            />
          </label>
          <label className="text-[10px] font-semibold uppercase tracking-[0.08em] text-black/45">
            Date
            <input
              type="date"
              value={eventDate}
              onChange={(event) => setEventDate(event.target.value)}
              className="mt-1 h-8 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/60 px-2 text-[12px] font-normal normal-case tracking-normal text-[var(--portal-navy)] outline-none"
            />
          </label>
          <label className="text-[10px] font-semibold uppercase tracking-[0.08em] text-black/45">
            Start
            <input
              type="time"
              value={startTime}
              onChange={(event) => setStartTime(event.target.value)}
              className="mt-1 h-8 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/60 px-2 text-[12px] font-normal normal-case tracking-normal text-[var(--portal-navy)] outline-none"
            />
          </label>
          <label className="text-[10px] font-semibold uppercase tracking-[0.08em] text-black/45">
            End
            <input
              type="time"
              value={endTime}
              onChange={(event) => setEndTime(event.target.value)}
              className="mt-1 h-8 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/60 px-2 text-[12px] font-normal normal-case tracking-normal text-[var(--portal-navy)] outline-none"
            />
          </label>
          <button
            type="button"
            disabled={queueing}
            onClick={queueEvent}
            className="h-8 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[11px] font-medium text-white transition hover:opacity-90 disabled:opacity-50"
          >
            {queueing ? "Queueing…" : "Add to Apple"}
          </button>
        </div>
      ) : null}

      {createStatus ? <p className="mt-1 text-[11px] text-[var(--portal-blue-gray)]">{createStatus}</p> : null}

      {error ? (
        <p className="mt-2 text-[11px] text-[var(--portal-archive)]">{error}</p>
      ) : loading ? (
        <p className="mt-2 text-[11px] font-light text-black/40">Loading today’s calendar…</p>
      ) : todaysEvents.length === 0 ? (
        <p className="mt-2 text-[11px] font-light text-black/40">No calendar events today.</p>
      ) : (
        <div className="mt-2 flex gap-2 overflow-x-auto pb-1">
          {todaysEvents.map((event) => (
            <article
              key={event.id}
              className="min-w-[210px] max-w-[300px] flex-1 rounded-[var(--portal-tab-radius)] border border-white/45 bg-white/30 px-3 py-2"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="flex items-center gap-1 text-[11px] font-medium text-[var(--portal-navy)]">
                  <Clock3 className="h-3 w-3 text-[var(--portal-gold-muted)]" aria-hidden />
                  {event.allDay ? "All day" : calendarTimeLabel(event.startAt)}
                </span>
                <span className="truncate text-[9px] font-medium uppercase tracking-[0.08em] text-black/35">
                  {sourceLabel(event.source)}
                </span>
              </div>
              <p className="mt-1 truncate text-[13px] font-medium text-[var(--portal-navy)]" title={event.title}>{event.title}</p>
              {event.personName || event.propertyName ? (
                <p className="mt-0.5 truncate text-[10px] font-light text-black/45">
                  {[event.personName, event.propertyName].filter(Boolean).join(" · ")}
                </p>
              ) : null}
            </article>
          ))}
        </div>
      )}
    </section>
  )
}
