'use client'

import { useEffect, useMemo, useState } from 'react'
import { IlamyCalendar, useIlamyCalendarContext } from '@ilamy/calendar'
import dayjs from 'dayjs'

import { Panel } from '@/components/portal/panel'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — calendar pane (real calendar engine, Apple Calendar–like).
//
// Uses @ilamy/calendar (React 19, Tailwind v4 / shadcn, MIT) for a genuine
// Month / Week / Day calendar. Apple Calendar remains authoritative; this pane
// renders the NORMALIZED CalendarEventSource projection — the library receives
// only normalized application events, never Apple / provider / database types.
// The header (month title, prev / today / next, DAY | WEEK | MONTH) is owned
// here via the library's `headerComponent` slot and skinned for CulebraLuxe.
// ---------------------------------------------------------------------------

const NAV_BTN =
  'inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]'

const VIEWS = [
  { key: 'day', label: 'DAY' },
  { key: 'week', label: 'WEEK' },
  { key: 'month', label: 'MONTH' },
] as const

function viewBtnClass(active: boolean) {
  return [
    'inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] border px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] transition',
    active
      ? 'border-[var(--portal-navy)] bg-[var(--portal-navy)] text-white'
      : 'border-[var(--portal-panel-border)] text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]',
  ].join(' ')
}

// Rendered by @ilamy/calendar inside its provider, so the context hook works.
function CalendarHeader() {
  const { currentDate, view, setView, prevPeriod, nextPeriod, today } =
    useIlamyCalendarContext()

  return (
    <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
      <div className="font-serif text-xl font-light text-[var(--portal-navy)]">
        {currentDate.format('MMMM YYYY')}
      </div>
      <div className="flex flex-wrap items-center gap-1.5">
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={prevPeriod}
            aria-label="Previous"
            className={NAV_BTN}
          >
            ←
          </button>
          <button type="button" onClick={today} className={NAV_BTN}>
            Today
          </button>
          <button
            type="button"
            onClick={nextPeriod}
            aria-label="Next"
            className={NAV_BTN}
          >
            →
          </button>
        </div>
        <div className="flex items-center gap-1">
          {VIEWS.map((v) => (
            <button
              key={v.key}
              type="button"
              onClick={() => setView(v.key)}
              className={viewBtnClass(view === v.key)}
            >
              {v.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}

function toCalendarEvent(event: CatchUpCalendarEvent) {
  const start = event.startAt
  const end =
    event.endAt ?? dayjs(event.startAt).add(1, 'hour').toISOString()
  const gold = event.kind === 'showing'
  return {
    id: event.id,
    title: event.title,
    start,
    end,
    allDay: event.allDay,
    color: gold ? '#c6a15b' : '#3f6ea5',
    backgroundColor: gold ? 'rgba(198,161,91,0.18)' : 'rgba(63,110,165,0.14)',
  }
}

export function CatchUpCalendar({
  events,
}: {
  events: CatchUpCalendarEvent[]
}) {
  const [mounted, setMounted] = useState(false)
  const calendarEvents = useMemo(() => events.map(toCalendarEvent), [events])

  useEffect(() => {
    setMounted(true)
  }, [])

  return (
    <Panel compact heading="Calendar" className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1">
        {!mounted ? (
          <div className="flex h-[28rem] items-center justify-center text-sm font-light text-black/40">
            Loading calendar…
          </div>
        ) : (
          <IlamyCalendar
            events={calendarEvents}
            initialView="month"
            initialDate={new Date()}
            headerComponent={<CalendarHeader />}
            firstDayOfWeek="sunday"
            hideExportButton
            disableCellClick
            disableEventClick
            disableDragAndDrop
            dayMaxEvents={4}
          />
        )}
      </div>
    </Panel>
  )
}
