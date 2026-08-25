'use client'

import { useState } from 'react'

import {
  CommandStatus,
  CommandStatusBand,
} from '@/components/portal/command-status-band'
import type { CommandStatusTone } from '@/components/portal/command-status-band'
import { CatchUpCommand } from '@/components/portal/catch-up-command'
import { CatchUpQueue } from '@/components/portal/catch-up-queue'
import { CatchUpCalendar } from '@/components/portal/catch-up-calendar'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — CORE daily destination.
//
//   COMMAND / ARA  |  STATUS
//   CATCH-UP QUEUE |  CALENDAR
//
// Reuses the shared CommandStatusBand (wide-command) — no local copy, no
// duplicated Forms/Clients implementation. The queue answers WHO needs Lisa and
// WHY; the calendar answers what is already scheduled. Responsive: band stacks
// (command above status), then queue above calendar.
// ---------------------------------------------------------------------------

export function CatchUp({
  calendarEvents,
  statusCue,
}: {
  calendarEvents: CatchUpCalendarEvent[]
  statusCue: string
}) {
  const [statusText, setStatusText] = useState(statusCue)
  const [statusTone, setStatusTone] = useState<CommandStatusTone>('neutral')

  return (
    <div className="flex flex-col gap-3">
      <CommandStatusBand
        ratio="wide-command"
        command={
          <CatchUpCommand
            onRun={(prompt) => {
              setStatusText(
                `“${prompt}” noted — Ara's deeper understanding is staged.`,
              )
              setStatusTone('neutral')
            }}
          />
        }
        status={
          <CommandStatus label="Status" tone={statusTone}>
            {statusText}
          </CommandStatus>
        }
      />

      <div className="grid gap-3 lg:grid-cols-2">
        <CatchUpQueue />
        <CatchUpCalendar events={calendarEvents} />
      </div>
    </div>
  )
}
