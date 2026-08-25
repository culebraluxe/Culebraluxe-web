'use client'

import { useEffect, useState } from 'react'
import Calendar from '@fullcalendar/react'
import daygrid from '@fullcalendar/react/daygrid'
import timegrid from '@fullcalendar/react/timegrid'
import '@fullcalendar/react/skeleton.css'
import '@fullcalendar/react/themes/classic/theme.css'
import '@fullcalendar/react/themes/classic/palette.css'

import { Panel } from '@/components/portal/panel'
import { toFullCalendarEvent } from '@/lib/catchup/calendar-mappers'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — calendar candidate: OPTION B (FullCalendar, free/open-source).
//
// FullCalendar v7 React (MIT core/daygrid/timegrid). Real Month / Week / Day
// time-grid views. Consumes the SAME normalized events as Option A via the
// shared mapper. The CulebraLuxe skin is applied by overriding the classic
// theme's --fc-classic-* variables on the `.cue-fc` container (see globals.css).
// ---------------------------------------------------------------------------

export function FullCalendarCandidate({
  events,
  heading = 'Calendar',
}: {
  events: CatchUpCalendarEvent[]
  heading?: string
}) {
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
  }, [])

  return (
    <Panel compact heading={heading} className="flex h-full min-h-0 flex-col">
      <div className="cue-fc h-[34rem]">
        {!mounted ? (
          <div className="flex h-full items-center justify-center text-sm font-light text-black/40">
            Loading calendar…
          </div>
        ) : (
          <Calendar
            plugins={[daygrid, timegrid]}
            initialView="dayGridMonth"
            initialDate={new Date()}
            events={events.map(toFullCalendarEvent)}
            headerToolbar={{
              left: 'prev,next today',
              center: 'title',
              right: 'dayGridMonth,timeGridWeek,timeGridDay',
            }}
            height="100%"
            dayMaxEvents={4}
            slotMinTime="08:00:00"
            slotMaxTime="20:00:00"
          />
        )}
      </div>
    </Panel>
  )
}
