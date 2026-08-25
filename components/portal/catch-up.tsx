'use client'

import { useState } from 'react'

import {
  CommandStatus,
  CommandStatusBand,
} from '@/components/portal/command-status-band'
import type { CommandStatusTone } from '@/components/portal/command-status-band'
import { CatchUpCommand } from '@/components/portal/catch-up-command'
import { CatchUpQueue } from '@/components/portal/catch-up-queue'
import { CatchUpCalendarEvaluation } from '@/components/portal/catch-up-calendar-evaluation'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — CORE daily destination.
//
//   COMMAND / ARA  |  STATUS
//   CATCH-UP QUEUE |  CALENDAR
//
// Reuses the shared CommandStatusBand (balanced) — no local copy, no
// duplicated Forms/Clients implementation. The balanced 50/50 ratio lines the
// COMMAND | STATUS seam up with the CATCH-UP QUEUE | CALENDAR row beneath it.
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
        ratio="balanced"
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

      <div className="grid gap-3 lg:grid-cols-2 lg:gap-4">
        <CatchUpQueue />
        {/* CAL-02 — temporary A/B evaluation (Option A ilamy, Option B
            FullCalendar). Same normalized events, stacked vertically. */}
        <CatchUpCalendarEvaluation events={calendarEvents} />
      </div>
    </div>
  )
}
