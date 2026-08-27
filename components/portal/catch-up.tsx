'use client'

import { useState } from 'react'

import {
  CommandStatus,
  CommandStatusBand,
} from '@/components/portal/command-status-band'
import type { CommandStatusTone } from '@/components/portal/command-status-band'
import { CatchUpCommand } from '@/components/portal/catch-up-command'
import { CatchUpWorkQueue } from '@/components/portal/catch-up-work-queue'
import { CatchUpTaskDetail } from '@/components/portal/catch-up-task-detail'
import { FullCalendarCandidate } from '@/components/portal/fullcalendar-candidate'
import { firstCatchUpTaskId } from '@/lib/catchup/task-tree'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'
import type { CatchUpTask } from '@/db/tasks'

// ---------------------------------------------------------------------------
// CATCH-UP — CORE daily destination (three-pane operator layout).
//
//   COMMAND / ARA  |  STATUS
//   NAVIGATOR  |  TASK WORKSPACE  |  CALENDAR
//
// Reuses the shared CommandStatusBand (balanced) — no local copy, no duplicated
// Forms/Clients implementation.
//
// Three sibling panes, roughly 20 / 40 / 40:
//   NAVIGATOR      — flat work-queue (workstream selector + category groups +
//                     selectable task rows). Fast to scan, thin, never in the way.
//   TASK WORKSPACE — the selected task is the star (enriched Post-it).
//   CALENDAR       — FullCalendar (Option B won) as THE right-side scheduling tool.
//
// Selection is lifted here: clicking / Entering a task in the navigator updates
// selectedTaskId, driving the navigator highlight and the middle workspace.
// Scanning with UP/DOWN moves focus WITHOUT changing the selected task, so the
// middle pane stays put until ENTER / click confirms a task.
// ---------------------------------------------------------------------------

export function CatchUp({
  calendarEvents,
  statusCue,
  tasks,
}: {
  calendarEvents: CatchUpCalendarEvent[]
  statusCue: string
  tasks: CatchUpTask[]
}) {
  const [statusText, setStatusText] = useState(statusCue)
  const [statusTone, setStatusTone] = useState<CommandStatusTone>('neutral')

  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(() =>
    firstCatchUpTaskId(
      tasks.map((task) => ({
        id: task.id,
        title: task.title,
        workstream: task.workstream ?? '',
        category: task.category ?? null,
      })),
    ),
  )
  const selectedTask =
    tasks.find((task) => task.id === selectedTaskId) ?? null

  return (
    <div className="flex flex-col gap-3">
      <CommandStatusBand
        ratio="command-6040"
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

      <div className="grid min-h-0 grid-cols-1 gap-3 lg:h-[36rem] lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)_minmax(0,2fr)] lg:gap-4">
        <CatchUpWorkQueue
          tasks={tasks}
          selectedTaskId={selectedTaskId}
          onSelectTask={setSelectedTaskId}
        />
        <CatchUpTaskDetail task={selectedTask} />
        {/* OPTION B (FullCalendar) is the ONE production Catch-Up calendar.
            Option A (@ilamy/calendar) is preserved but no longer rendered. */}
        <FullCalendarCandidate events={calendarEvents} heading="Calendar" />
      </div>
    </div>
  )
}
