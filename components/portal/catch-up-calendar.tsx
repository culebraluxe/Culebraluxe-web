'use client'

import { useEffect, useMemo, useState } from 'react'
import { IlamyCalendar, useIlamyCalendarContext } from '@ilamy/calendar'

import { Panel } from '@/components/portal/panel'
import { toIlamyCalendarEvent } from '@/lib/catchup/calendar-mappers'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — calendar candidate: OPTION A (current incumbent, @ilamy/calendar).
//
// Real Month / Week / Day engine (React 19, Tailwind v4 / shadcn, MIT). Renders
// the NORMALIZED CalendarEventSource projection via the shared mapper — same
// events the FullCalendar candidate consumes. Apple Calendar remains
// authoritative; this pane never touches Apple/provider/database types.
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

export function CatchUpCalendar({
  events,
  heading = 'Calendar',
}: {
  events: CatchUpCalendarEvent[]
  heading?: string
}) {
  const [mounted, setMounted] = useState(false)
  const calendarEvents = useMemo(
    () => events.map(toIlamyCalendarEvent),
    [events],
  )

  useEffect(() => {
    setMounted(true)
  }, [])

  return (
    <Panel compact heading={heading} className="flex h-full min-h-0 flex-col">
      <div className="h-[34rem]">
        {!mounted ? (
          <div className="flex h-full items-center justify-center text-sm font-light text-black/40">
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
