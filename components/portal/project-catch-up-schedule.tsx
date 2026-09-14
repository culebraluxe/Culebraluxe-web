"use client"

import { useEffect, useMemo, useState } from "react"
import { CalendarDays, Clock3 } from "lucide-react"

import { loadCatchUpScheduleAction } from "@/app/portal/projects/calendar-actions"
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

export function ProjectCatchUpSchedule({ today }: { today: Date | null }) {
  const [events, setEvents] = useState<CatchUpCalendarEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

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
  const todaysEvents = useMemo(
    () => events
      .filter((event) => todayKey && puertoRicoDateKey(event.startAt) === todayKey)
      .sort((a, b) => {
        if (a.allDay !== b.allDay) return a.allDay ? -1 : 1
        return a.startAt.localeCompare(b.startAt) || a.title.localeCompare(b.title)
      }),
    [events, todayKey],
  )

  return (
    <section className="shrink-0 border-b border-[var(--portal-panel-border)]/70 px-3 py-2" aria-label="Today's schedule">
      <div className="flex items-center gap-2">
        <CalendarDays className="h-4 w-4 shrink-0 text-[var(--portal-gold-muted)]" aria-hidden />
        <span className="text-[10px] font-semibold uppercase tracking-[0.12em] text-black/40">Schedule</span>
        <span className="rounded-full bg-white/40 px-2 py-0.5 text-[10px] font-medium text-[var(--portal-navy-soft)]">
          {loading ? "…" : todaysEvents.length}
        </span>
        <span className="text-[10px] font-light text-black/35">Calendar events · read-only</span>
      </div>

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
