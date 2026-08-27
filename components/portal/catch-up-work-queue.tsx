'use client'

import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from 'react'

import { Panel } from '@/components/portal/panel'
import {
  buildCatchUpNavRows,
  getCatchUpWorkstreams,
  type CatchUpNavRow,
} from '@/lib/catchup/task-tree'
import type { CatchUpTask } from '@/db/tasks'

// ---------------------------------------------------------------------------
// CATCH-UP — Work-queue NAVIGATOR (left pane, three-pane layout).
//
// A FLAT, grouped work navigator — deliberately NOT a tree. Indentation stole
// horizontal room from long task titles, so this pane:
//   TAX 1  workstream SELECTOR  (CLIENT | CORE | OPPS | SUPPORT | TECH)
//   TAX 2  category SECTION HEADER (quiet)
//   TASK   selectable FLAT row  (title only, nearly full width)
//
// Only one workstream is shown at a time. Task rows are flat (no indentation),
// no counts, no owner/status/due/buttons. The task TITLE is the star and is
// never truncated aggressively — it wraps.
//
// Keyboard (on the list region):
//   UP / DOWN  move between TASK rows (category headers are not selectable)
//   ENTER      select/activate the focused task → loads the Task Workspace
//   HOME / END jump to first / last task
// Mouse click selects a task immediately.
// ---------------------------------------------------------------------------

// The workstream selector uses the shared compact glass-rail material (see
// .portal-glass-rail--compact / .portal-glass-tab--compact in globals.css): a
// single floating glass control with navy active + restrained gold cue, instead
// of five unrelated outlined buttons.

export function CatchUpWorkQueue({
  tasks,
  selectedTaskId,
  onSelectTask,
}: {
  tasks: CatchUpTask[]
  selectedTaskId: string | null
  onSelectTask: (taskId: string | null) => void
}) {
  const leafTasks = useMemo(
    () =>
      tasks.map((t) => ({
        id: t.id,
        title: t.title,
        workstream: t.workstream ?? '',
        category: t.category ?? null,
      })),
    [tasks],
  )
  const workstreams = useMemo(
    () => getCatchUpWorkstreams(leafTasks),
    [leafTasks],
  )
  const [activeWorkstream, setActiveWorkstream] = useState<string | null>(
    () => workstreams[0] ?? null,
  )

  const rows = useMemo(
    () =>
      activeWorkstream
        ? buildCatchUpNavRows(leafTasks, activeWorkstream)
        : ([] as CatchUpNavRow[]),
    [leafTasks, activeWorkstream],
  )

  // Task-only rows drive the selection sequence (category headers are skipped).
  const taskRows = useMemo(
    () =>
      rows.filter(
        (r): r is Extract<CatchUpNavRow, { kind: 'task' }> => r.kind === 'task',
      ),
    [rows],
  )
  const taskIndex = useMemo(() => {
    const map = new Map<string, number>()
    taskRows.forEach((row, i) => map.set(row.id, i))
    return map
  }, [taskRows])

  const [focusedIndex, setFocusedIndex] = useState(0)
  const listRef = useRef<HTMLDivElement>(null)
  const rowRefs = useRef<Record<string, HTMLDivElement | null>>({})

  // Keep the focus index in range when the workstream's task list changes.
  useEffect(() => {
    if (taskRows.length === 0) return
    if (focusedIndex >= taskRows.length) {
      setFocusedIndex(Math.max(0, taskRows.length - 1))
    }
  }, [taskRows.length, focusedIndex])

  // Keep the focused task visible while moving through the list.
  useEffect(() => {
    const id = taskRows[focusedIndex]?.id
    if (id) rowRefs.current[id]?.scrollIntoView({ block: 'nearest' })
  }, [focusedIndex, taskRows])

  // Focus the list region when the workstream changes so arrows work at once.
  useEffect(() => {
    listRef.current?.focus()
  }, [activeWorkstream])

  function selectWorkstream(ws: string) {
    if (ws === activeWorkstream) return
    setActiveWorkstream(ws)
    setFocusedIndex(0)
    listRef.current?.scrollTo({ top: 0 })
    const first = buildCatchUpNavRows(leafTasks, ws).find((r) => r.kind === 'task')
    onSelectTask(first ? first.id : null)
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (taskRows.length === 0) return
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        setFocusedIndex((i) => Math.min(i + 1, taskRows.length - 1))
        break
      case 'ArrowUp':
        event.preventDefault()
        setFocusedIndex((i) => Math.max(i - 1, 0))
        break
      case 'Home':
        event.preventDefault()
        setFocusedIndex(0)
        break
      case 'End':
        event.preventDefault()
        setFocusedIndex(taskRows.length - 1)
        break
      case 'Enter': {
        event.preventDefault()
        const id = taskRows[focusedIndex]?.id
        if (id) onSelectTask(id)
        break
      }
    }
  }

  function handleRowClick(id: string) {
    const idx = taskIndex.get(id)
    if (idx !== undefined) setFocusedIndex(idx)
    onSelectTask(id)
    listRef.current?.focus()
  }


  return (
    <Panel compact lifted heading="Catch-Up" className="flex h-full min-h-0 min-w-0 flex-col">
      {workstreams.length === 0 ? (
        <p className="flex flex-1 items-center justify-center px-4 text-center text-sm font-light text-black/40">
          No active workstream tasks right now.
        </p>
      ) : (
        <>
          <div
            className="portal-glass-rail portal-glass-rail--compact mb-3"
            role="tablist"
            aria-label="Workstreams"
          >
            {workstreams.map((ws) => {
              const active = ws === activeWorkstream
              return (
                <button
                  key={ws}
                  type="button"
                  role="tab"
                  aria-selected={active}
                  aria-current={active ? 'page' : undefined}
                  onClick={() => selectWorkstream(ws)}
                  className="portal-glass-tab portal-glass-tab--compact"
                >
                  {ws}
                </button>
              )
            })}
          </div>

          <div
            ref={listRef}
            tabIndex={0}
            role="listbox"
            aria-label="Catch-Up work queue"
            onKeyDown={handleKeyDown}
            className="min-h-0 flex-1 overflow-y-auto rounded-md focus:outline-none"
          >
            {rows.map((row) => {
              if (row.kind === 'category') {
                return (
                  <div
                    key={row.id}
                    role="presentation"
                    className="px-2 pb-1 pt-3 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-panel-heading-muted)]"
                  >
                    {row.name}
                  </div>
                )
              }
              const selected = row.id === selectedTaskId
              const focused = row.id === taskRows[focusedIndex]?.id
              return (
                <div
                  key={row.id}
                  role="option"
                  aria-selected={selected}
                  ref={(el) => {
                    rowRefs.current[row.id] = el
                  }}
                  onClick={() => handleRowClick(row.id)}
                  className={[
                    'cursor-pointer rounded-md px-2 py-2 transition-colors',
                    selected
                      ? 'bg-[var(--portal-gold-pale)] text-[var(--portal-navy)] shadow-[inset_0_1px_0_rgba(255,255,255,0.55)]'
                      : focused
                        ? 'bg-black/[0.04] text-[var(--portal-navy)]'
                        : 'text-[var(--portal-navy)] hover:bg-black/[0.03]',
                  ].join(' ')}
                >
                  <span className="block break-words text-[14px] leading-snug">
                    {row.title}
                  </span>
                </div>
              )
            })}
          </div>
        </>
      )}
    </Panel>
  )
}

