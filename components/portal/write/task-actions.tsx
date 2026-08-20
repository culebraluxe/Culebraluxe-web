'use client'

import { useState, useTransition } from 'react'

import {
  cancelTaskAction,
  completeTaskAction,
  createTaskAction,
  updateTaskDueAction,
} from '@/app/portal/actions'

type Result = { ok: boolean; message?: string }

function resolve(result: unknown): Result {
  const value = result as Result
  return value
}

const secondaryButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40'

const ghostButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm px-3 text-[11px] font-light uppercase tracking-[0.14em] text-black/45 transition hover:text-[#8a4b2a] disabled:cursor-not-allowed disabled:opacity-40'

export function TaskActions({ taskId }: { taskId: string }) {
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<string | null>(null)

  function run(action: () => Promise<unknown>) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await action())
      if (!result.ok) setMessage(result.message ?? 'Action failed.')
    })
  }

  return (
    <div className="flex flex-wrap items-center gap-2">
      <button
        type="button"
        disabled={isPending}
        onClick={() => run(() => completeTaskAction(taskId))}
        className={secondaryButton}
      >
        Complete
      </button>
      <button
        type="button"
        disabled={isPending}
        onClick={() => run(() => cancelTaskAction(taskId))}
        className={ghostButton}
      >
        Cancel
      </button>
      {message && (
        <span className="text-xs font-light text-[#8a4b2a]">{message}</span>
      )}
    </div>
  )
}

export function CreateTaskForm({
  personId,
  propertyId,
  dealId,
  compact = false,
}: {
  personId?: string
  propertyId?: string
  dealId?: string
  compact?: boolean
}) {
  const [isPending, startTransition] = useTransition()
  const [title, setTitle] = useState('')
  const [dueAt, setDueAt] = useState('')
  const [message, setMessage] = useState<{
    ok: boolean
    text: string
  } | null>(null)

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await createTaskAction({
          title,
          dueAt: dueAt || null,
          personId,
          propertyId,
          dealId,
        }),
      )
      if (result.ok) {
        setTitle('')
        setDueAt('')
        setMessage({ ok: true, text: 'Task created.' })
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not create task.' })
      }
    })
  }

  return (
    <form onSubmit={submit} className={compact ? 'space-y-3' : 'space-y-3 border-t border-[var(--portal-border)] px-6 py-5'}>
      <div className="grid gap-3 md:grid-cols-2">
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Task
          </span>
          <input
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            required
            placeholder="Follow up on X"
            className="mt-2 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          />
        </label>
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Due (optional)
          </span>
          <input
            type="datetime-local"
            value={dueAt}
            onChange={(event) => setDueAt(event.target.value)}
            className="mt-2 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          />
        </label>
      </div>
      <div className="flex items-center gap-3">
        <button
          type="submit"
          disabled={isPending}
          className="inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-4 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"
        >
          Add task
        </button>
        {message && (
          <span
            className={`text-xs font-light ${
              message.ok ? 'text-black/50' : 'text-[#8a4b2a]'
            }`}
          >
            {message.text}
          </span>
        )}
      </div>
    </form>
  )
}

export function UpdateTaskDue({ taskId }: { taskId: string }) {
  const [isPending, startTransition] = useTransition()
  const [dueAt, setDueAt] = useState('')
  const [message, setMessage] = useState<string | null>(null)

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await updateTaskDueAction(taskId, dueAt || null),
      )
      if (!result.ok) setMessage(result.message ?? 'Could not update due date.')
      else setDueAt('')
    })
  }

  return (
    <form onSubmit={submit} className="flex items-center gap-2">
      <input
        type="datetime-local"
        value={dueAt}
        onChange={(event) => setDueAt(event.target.value)}
        aria-label="Due date"
        className="block min-h-11 w-48 rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
      />
      <button
        type="submit"
        disabled={isPending}
        className={secondaryButton}
      >
        Set due
      </button>
      {message && (
        <span className="text-xs font-light text-[#8a4b2a]">{message}</span>
      )}
    </form>
  )
}
