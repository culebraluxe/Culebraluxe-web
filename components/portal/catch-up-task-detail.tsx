'use client'

import { useActionState, useEffect, useMemo, useRef, useState } from 'react'

import { Panel } from '@/components/portal/panel'
import { ghostBtn } from '@/components/portal/client-display'
import {
  completeTaskAction,
  createTaskAction,
  saveTaskAction,
} from '@/app/portal/catch-up/actions'
import {
  CATCHUP_WORKSTREAMS,
  categoriesForWorkstream,
} from '@/lib/catchup/task-taxonomy'
import {
  PRIORITY_LEVELS,
  priorityToLevel,
} from '@/lib/catchup/task-priority'
import type { CatchUpTask } from '@/db/tasks'

// ---------------------------------------------------------------------------
// CATCH-UP — Task Workspace (center pane, three-pane layout).
//
// The visual/anchor surface. A deep translucent NAVY Feature panel with a
// lighter floating task card inside — an "enriched Post-it note". The workspace
// opens ALREADY EDITABLE (no Edit gate): Title, Notes/Detail, Target Date,
// Workstream, Category, and Priority are always editable. Add Date (task.created_at)
// is read-only/system-owned. SAVE persists edits and keeps the task active;
// COMPLETE performs the canonical completion and removes it from the queue.
//
// + NEW TASK flips the same center panel into a blank creation form; CREATE TASK
// writes a real canonical task through the existing createTask seam (never a
// UI-only row), then the created task appears in the navigator and is selected.
//
// Persistence reuses the canonical task write service via the Catch-Up server
// actions (updateTask / createTask / completeTask). No second task subsystem.
// ---------------------------------------------------------------------------

type TaskFormFields = {
  id: string
  title: string
  detail: string | null
  dueAt: string | null
  priority: number
  workstream: string
  category: string | null
}

/** ISO date → short human-readable date (e.g. "Aug 27, 2026"). */
function formatDate(iso: string | null): string {
  if (!iso) return ''
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return date.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  })
}

/** ISO → datetime-local input value (local time), '' when absent. */
function toLocalInput(iso: string | null): string {
  if (!iso) return ''
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return ''
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(
    date.getDate(),
  )}T${pad(date.getHours())}:${pad(date.getMinutes())}`
}

/** Today at local midnight, as a datetime-local input value. */
function todayLocal(): string {
  const date = new Date()
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(
    date.getDate(),
  )}T00:00`
}

// High-contrast neutral labels (not gold) on the pale writing surface.
const labelCls =
  'block text-[10px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-navy-soft)]'

const inputCls =
  'block w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-3 py-2 text-sm font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]'

const textareaCls =
  'block min-h-[12rem] w-full resize-y overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-3 py-2.5 text-[15px] font-light leading-6 text-[var(--portal-navy)] outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]'

const readOnlyCls =
  'mt-1.5 block w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/50 px-3 py-2 text-sm font-light text-[var(--portal-navy-soft)]'

const btnPrimary =
  'inline-flex h-9 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-50'

const btnComplete =
  'inline-flex h-9 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.18)] transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-50'

export function CatchUpTaskDetail({
  task,
  mode,
  defaultWorkstream,
  defaultCategory,
  onDirtyChange,
  onSaved,
  onCompleted,
  onCreate,
  onCreateCancel,
  onNewTask,
}: {
  task: CatchUpTask | null
  mode: 'edit' | 'create'
  defaultWorkstream: string | null
  defaultCategory: string | null
  onDirtyChange: (dirty: boolean) => void
  onSaved: (fields: TaskFormFields) => void
  onCompleted: (taskId: string) => void
  onCreate: (fields: TaskFormFields) => void
  onCreateCancel: () => void
  onNewTask: () => void
}) {
  const [title, setTitle] = useState('')
  const [detail, setDetail] = useState('')
  const [targetDate, setTargetDate] = useState('')
  const [workstream, setWorkstream] = useState('')
  const [category, setCategory] = useState<string | null>(null)
  const [priority, setPriority] = useState(1)
  const [dirty, setDirty] = useState(false)

  const titleRef = useRef<HTMLInputElement>(null)

  const [saveState, saveFormAction, savePending] = useActionState(
    saveTaskAction,
    null,
  )
  const [createState, createFormAction, createPending] = useActionState(
    createTaskAction,
    null,
  )
  const [completeState, completeFormAction, completePending] = useActionState(
    completeTaskAction,
    null,
  )

  // Form sync: initialize/reset when mode or the selected task changes.
  useEffect(() => {
    setDirty(false)
    onDirtyChange(false)
    if (mode === 'create') {
      const ws = defaultWorkstream ?? CATCHUP_WORKSTREAMS[0]
      setTitle('')
      setDetail('')
      setTargetDate(todayLocal())
      setWorkstream(ws)
      setCategory(defaultCategory ?? categoriesForWorkstream(ws)[0] ?? null)
      setPriority(1)
    } else if (task) {
      const ws = task.workstream ?? defaultWorkstream ?? CATCHUP_WORKSTREAMS[0]
      setTitle(task.title)
      setDetail(task.detail ?? '')
      setTargetDate(toLocalInput(task.dueAt))
      setWorkstream(ws)
      setCategory(task.category ?? categoriesForWorkstream(ws)[0] ?? null)
      setPriority(priorityToLevel(task.priority))
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, task?.id, onDirtyChange])

  // Focus the title when entering new-task mode.
  useEffect(() => {
    if (mode === 'create') titleRef.current?.focus()
  }, [mode])

  // Apply a successful save.
  useEffect(() => {
    if (!saveState || !saveState.ok || !saveState.taskId) return
    setDirty(false)
    onDirtyChange(false)
    onSaved({
      id: saveState.taskId,
      title: saveState.title ?? '',
      detail: saveState.detail ?? null,
      dueAt: saveState.dueAt ?? null,
      priority: saveState.priority ?? 0,
      workstream: saveState.workstream ?? '',
      category: saveState.category ?? null,
    })
  }, [saveState, onDirtyChange, onSaved])

  // Apply a successful create.
  useEffect(() => {
    if (!createState || !createState.ok || !createState.taskId) return
    setDirty(false)
    onDirtyChange(false)
    onCreate({
      id: createState.taskId,
      title: createState.title ?? '',
      detail: createState.detail ?? null,
      dueAt: createState.dueAt ?? null,
      priority: createState.priority ?? 1,
      workstream: createState.workstream ?? '',
      category: createState.category ?? null,
    })
  }, [createState, onDirtyChange, onCreate])

  // Apply a successful completion.
  useEffect(() => {
    if (!completeState || !completeState.ok || !completeState.taskId) return
    setDirty(false)
    onDirtyChange(false)
    onCompleted(completeState.taskId)
  }, [completeState, onDirtyChange, onCompleted])

  function markDirty() {
    if (!dirty) {
      setDirty(true)
      onDirtyChange(true)
    }
  }

  function onWorkstreamChange(ws: string) {
    setWorkstream(ws)
    const cats = categoriesForWorkstream(ws)
    if (!cats.includes(category ?? '')) setCategory(cats[0] ?? null)
    markDirty()
  }

  const workstreamOptions = useMemo(() => {
    if (
      workstream &&
      !CATCHUP_WORKSTREAMS.includes(
        workstream as (typeof CATCHUP_WORKSTREAMS)[number],
      )
    ) {
      return [workstream, ...CATCHUP_WORKSTREAMS]
    }
    return CATCHUP_WORKSTREAMS
  }, [workstream])

  const categoryOptions = useMemo(() => {
    const supported = categoriesForWorkstream(workstream)
    if (category && !supported.includes(category)) return [category, ...supported]
    return supported
  }, [workstream, category])

  const addDate =
    mode === 'create' ? new Date().toISOString() : (task?.createdAt ?? null)


  const newTaskButton = (
    <button
      type="button"
      onClick={onNewTask}
      disabled={mode === 'create'}
      className="inline-flex h-9 items-center rounded-[var(--portal-tab-radius)] border border-white/20 bg-white/10 px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-white/90 transition hover:border-white/45 hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
    >
      + New Task
    </button>
  )

  // Shared field block — used by both the edit (save) and create forms so the
  // two surfaces cannot drift.
  const fieldsBlock = (
    <>
      <div>
        <label htmlFor="task-title" className={labelCls}>
          Title
        </label>
        <input
          id="task-title"
          ref={titleRef}
          name="title"
          value={title}
          onChange={(event) => {
            setTitle(event.target.value)
            markDirty()
          }}
          placeholder="What needs doing?"
          className={`mt-1.5 ${inputCls}`}
        />
      </div>

      <div>
        <label htmlFor="task-detail" className={labelCls}>
          Notes / Detail
        </label>
        <textarea
          id="task-detail"
          name="detail"
          value={detail}
          onChange={(event) => {
            setDetail(event.target.value)
            markDirty()
          }}
          placeholder="Write a freeform note — what happened, what’s next, who to ask."
          className={`mt-1.5 ${textareaCls}`}
        />
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <div>
          <label className={labelCls}>Add Date</label>
          <div className={readOnlyCls}>{formatDate(addDate)}</div>
        </div>
        <div>
          <label htmlFor="task-target" className={labelCls}>
            Target Date
          </label>
          <input
            id="task-target"
            type="datetime-local"
            name="targetDate"
            value={targetDate}
            onChange={(event) => {
              setTargetDate(event.target.value)
              markDirty()
            }}
            className={`mt-1.5 ${inputCls}`}
          />
        </div>
      </div>

      <div className="grid gap-4 sm:grid-cols-3">
        <div>
          <label htmlFor="task-workstream" className={labelCls}>
            Workstream
          </label>
          <select
            id="task-workstream"
            name="workstream"
            value={workstream}
            onChange={(event) => onWorkstreamChange(event.target.value)}
            className={`mt-1.5 ${inputCls}`}
          >
            {workstreamOptions.map((ws) => (
              <option key={ws} value={ws}>
                {ws}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label htmlFor="task-category" className={labelCls}>
            Category
          </label>
          <select
            id="task-category"
            name="category"
            value={category ?? ''}
            onChange={(event) => {
              setCategory(event.target.value || null)
              markDirty()
            }}
            className={`mt-1.5 ${inputCls}`}
          >
            {categoryOptions.map((cat) => (
              <option key={cat} value={cat}>
                {cat}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label htmlFor="task-priority" className={labelCls}>
            Priority
          </label>
          <select
            id="task-priority"
            name="priority"
            value={priority}
            onChange={(event) => {
              setPriority(Number(event.target.value))
              markDirty()
            }}
            className={`mt-1.5 ${inputCls}`}
          >
            {PRIORITY_LEVELS.map((level) => (
              <option key={level.value} value={level.value}>
                {level.label}
              </option>
            ))}
          </select>
        </div>
      </div>
    </>
  )


  return (
    <Panel
      compact
      variant="feature"
      lifted
      headingSize="xl"
      heading="Task Workspace"
      action={newTaskButton}
      className="flex h-full min-h-0 min-w-0 flex-col"
    >
      <div className="min-h-0 flex-1 overflow-y-auto">
        {mode === 'create' ? (
          <form
            action={createFormAction}
            className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5 text-[var(--portal-navy)]"
          >
            <div className="grid gap-5">{fieldsBlock}</div>
            <div className="mt-5 flex flex-wrap items-center gap-2 border-t border-[var(--portal-panel-border)] pt-4">
              <button
                type="submit"
                disabled={!title.trim() || createPending}
                className={btnPrimary}
              >
                Create Task
              </button>
              <button type="button" onClick={onCreateCancel} className={ghostBtn}>
                Cancel
              </button>
            </div>
            {createState?.error ? (
              <p className="mt-3 text-sm font-light text-[var(--portal-gold-muted)]">
                {createState.error}
              </p>
            ) : null}
          </form>
        ) : task ? (
          <div className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5 text-[var(--portal-navy)]">
            <form id="task-save" action={saveFormAction} className="grid gap-5">
              <input type="hidden" name="taskId" value={task.id} />
              {fieldsBlock}
            </form>
            <div className="mt-5 flex flex-wrap items-center gap-2 border-t border-[var(--portal-panel-border)] pt-4">
              <button form="task-save" type="submit" disabled={savePending} className={btnPrimary}>
                Save
              </button>
              <form action={completeFormAction}>
                <input type="hidden" name="taskId" value={task.id} />
                <button type="submit" disabled={completePending} className={btnComplete}>
                  Complete
                </button>
              </form>
            </div>
            {saveState?.error ? (
              <p className="mt-3 text-sm font-light text-[var(--portal-gold-muted)]">
                {saveState.error}
              </p>
            ) : null}
            {completeState?.error ? (
              <p className="mt-3 text-sm font-light text-[var(--portal-gold-muted)]">
                {completeState.error}
              </p>
            ) : null}
          </div>
        ) : (
          <p className="flex h-full items-center justify-center px-4 text-center font-serif text-lg font-light text-white/70">
            Select a task to begin, or tap + New Task.
          </p>
        )}
      </div>
    </Panel>
  )
}
