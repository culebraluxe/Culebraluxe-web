'use client'

import { useCallback, useMemo, useState } from 'react'

import {
  CommandStatus,
  CommandStatusBand,
} from '@/components/portal/command-status-band'
import type { CommandStatusTone } from '@/components/portal/command-status-band'
import { CatchUpCommand } from '@/components/portal/catch-up-command'
import { CatchUpWorkQueue } from '@/components/portal/catch-up-work-queue'
import { CatchUpTaskDetail } from '@/components/portal/catch-up-task-detail'
import { FullCalendarCandidate } from '@/components/portal/fullcalendar-candidate'
import {
  buildCatchUpNavRows,
  firstCatchUpTaskId,
  getCatchUpWorkstreams,
} from '@/lib/catchup/task-tree'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'
import type { CatchUpTask } from '@/db/tasks'

// ---------------------------------------------------------------------------
// CATCH-UP — CORE daily destination (three-pane operator layout).
//
//   COMMAND / ARA (60)  |  STATUS (40)
//   NAVIGATOR (20)  |  TASK WORKSPACE (40)  |  CALENDAR (40)
//
// Reuses the shared CommandStatusBand (command-6040) and the shared Panel /
// glass system — no local copies, no duplicated Forms/Clients implementation.
//
// Active tasks are held in client state so the operator workflow is immediate:
//   - the Task Workspace opens ALREADY editable (no Edit gate)
//   - SAVE persists edits canonically and reconciles the row
//   - COMPLETE persists canonically, removes the task from the active queue,
//     and the navigator selects the next sensible visible task
//   - + NEW TASK opens a blank creation form in the same center panel; CREATE
//     TASK writes a real canonical task, which then appears in the navigator
//     and becomes the selected task
// Unsaved edits cannot be silently lost: switching tasks / starting a new task
// while dirty asks for confirmation first.
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

  const [activeTasks, setActiveTasks] = useState<CatchUpTask[]>(tasks)
  const [unsavedChanges, setUnsavedChanges] = useState(false)
  const [creatingNew, setCreatingNew] = useState(false)

  const [activeWorkstream, setActiveWorkstream] = useState<string | null>(() => {
    const ws = getCatchUpWorkstreams(
      tasks.map((task) => ({
        id: task.id,
        title: task.title,
        workstream: task.workstream ?? '',
        category: task.category ?? null,
      })),
    )
    return ws[0] ?? null
  })

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
    activeTasks.find((task) => task.id === selectedTaskId) ?? null

  const leafTasks = useMemo(
    () =>
      activeTasks.map((task) => ({
        id: task.id,
        title: task.title,
        workstream: task.workstream ?? '',
        category: task.category ?? null,
      })),
    [activeTasks],
  )

  // NEW TASK default category: the selected task's category when it belongs to
  // the active workstream, else the first category section of that workstream.
  const defaultCategory = useMemo(() => {
    if (
      selectedTask &&
      selectedTask.workstream === activeWorkstream &&
      selectedTask.category
    ) {
      return selectedTask.category
    }
    const rows = activeWorkstream
      ? buildCatchUpNavRows(leafTasks, activeWorkstream)
      : []
    return rows.find((row) => row.kind === 'category')?.name ?? null
  }, [selectedTask, activeWorkstream, leafTasks])

  // Selection gate: never silently discard unsaved edits; selecting a task
  // always exits any in-progress new-task form.
  const handleSelectTask = useCallback(
    (taskId: string | null) => {
      if (taskId === selectedTaskId && !creatingNew) return
      if (
        unsavedChanges &&
        !window.confirm('Discard unsaved changes and switch tasks?')
      ) {
        return
      }
      setUnsavedChanges(false)
      setCreatingNew(false)
      setSelectedTaskId(taskId)
    },
    [selectedTaskId, unsavedChanges, creatingNew],
  )

  const handleNewTask = useCallback(() => {
    if (
      unsavedChanges &&
      !window.confirm('Discard unsaved changes and start a new task?')
    ) {
      return
    }
    setUnsavedChanges(false)
    setCreatingNew(true)
  }, [unsavedChanges])

  const handleCreateCancel = useCallback(() => {
    setUnsavedChanges(false)
    setCreatingNew(false)
  }, [])

  const handleSaved = useCallback(
    (fields: {
      id: string
      title: string
      detail: string | null
      dueAt: string | null
      priority: number
      workstream: string
      category: string | null
    }) => {
      setUnsavedChanges(false)
      setActiveTasks((prev) =>
        prev.map((task) =>
          task.id === fields.id
            ? {
                ...task,
                title: fields.title,
                detail: fields.detail,
                dueAt: fields.dueAt,
                priority: fields.priority,
                workstream: fields.workstream,
                category: fields.category,
              }
            : task,
        ),
      )
    },
    [],
  )

  // Completion is persisted via the canonical seam; here we just drop the row
  // from the active queue — the navigator re-selects the next sensible task.
  const handleCompleted = useCallback((taskId: string) => {
    setUnsavedChanges(false)
    setActiveTasks((prev) => prev.filter((task) => task.id !== taskId))
  }, [])

  // A real canonical task was created: add it to the active queue, make it the
  // selected task, and leave create mode so it can be edited/saved/completed.
  const handleCreated = useCallback(
    (fields: {
      id: string
      title: string
      detail: string | null
      dueAt: string | null
      priority: number
      workstream: string
      category: string | null
    }) => {
      setUnsavedChanges(false)
      setCreatingNew(false)
      const created: CatchUpTask = {
        id: fields.id,
        title: fields.title,
        workstream: fields.workstream,
        category: fields.category,
        status: 'open',
        priority: fields.priority,
        detail: fields.detail,
        createdAt: new Date().toISOString(),
        assignedUserId: null,
        ownerName: null,
        dueAt: fields.dueAt,
        personId: null,
        personName: null,
        propertyId: null,
        propertyName: null,
        dealId: null,
        dealName: null,
      }
      setActiveTasks((prev) => [...prev, created])
      setSelectedTaskId(fields.id)
    },
    [],
  )


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
          tasks={activeTasks}
          selectedTaskId={selectedTaskId}
          activeWorkstream={activeWorkstream}
          onSelectTask={handleSelectTask}
          onWorkstreamChange={setActiveWorkstream}
        />
        <CatchUpTaskDetail
          task={selectedTask}
          mode={creatingNew ? 'create' : 'edit'}
          defaultWorkstream={activeWorkstream}
          defaultCategory={defaultCategory}
          onDirtyChange={setUnsavedChanges}
          onSaved={handleSaved}
          onCompleted={handleCompleted}
          onCreate={handleCreated}
          onCreateCancel={handleCreateCancel}
          onNewTask={handleNewTask}
        />
        {/* OPTION B (FullCalendar) is the ONE production Catch-Up calendar.
            Option A (@ilamy/calendar) is preserved but no longer rendered. */}
        <FullCalendarCandidate events={calendarEvents} heading="Calendar" />
      </div>
    </div>
  )
}
