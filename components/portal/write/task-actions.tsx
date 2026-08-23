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
  return result as Result
}

const secondaryButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40'

const ghostButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm px-3 text-[11px] font-light uppercase tracking-[0.14em] text-black/45 transition hover:text-[var(--portal-archive)] disabled:cursor-not-allowed disabled:opacity-40'

export function TaskActions({
  taskId,
  compact = false,
}: {
  taskId: string
  compact?: boolean
}) {
  const [isPending, startTransition] = useTransition()
  const [dueAt, setDueAt] = useState('')
  const [message, setMessage] = useState<{
    ok: boolean
    text: string
  } | null>(null)

  function run(action: () => Promise<unknown>) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await action())
      if (!result.ok) {
        setMessage({ ok: false, text: result.message ?? 'Action failed.' })
      }
    })
  }

  function submitDue(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await updateTaskDueAction(taskId, dueAt || null))
      if (result.ok) {
        setDueAt('')
        setMessage({ ok: true, text: 'Due date saved.' })
      } else {
        setMessage({
          ok: false,
          text: result.message ?? 'Could not update due date.',
        })
      }
    })
  }

  if (compact) {
    return (
      <div className="flex flex-wrap items-center gap-1.5">
        <button
          type="button"
          disabled={isPending}
          onClick={() => run(() => completeTaskAction(taskId))}
          className="inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:opacity-40"
        >
          Done
        </button>
        {message && !message.ok ? (
          <span className="text-[10px] font-light text-[var(--portal-archive)]">
            {message.text}
          </span>
        ) : null}
      </div>
    )
  }

  return (
    <div className="space-y-2">
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
      </div>
      <form onSubmit={submitDue} className="flex flex-wrap items-center gap-2">
        <label className="sr-only" htmlFor={`due-${taskId}`}>
          Due date and time (local)
        </label>
        <input
          id={`due-${taskId}`}
          type="datetime-local"
          value={dueAt}
          onChange={(event) => setDueAt(event.target.value)}
          aria-label="Due date and time (local)"
          className="block min-h-11 w-52 rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
        />
        <button
          type="submit"
          disabled={isPending}
          className={secondaryButton}
        >
          Set due
        </button>
      </form>
      {message && (
        <span
          className={`text-xs font-light ${
            message.ok ? 'text-black/50' : 'text-[var(--portal-archive)]'
          }`}
        >
          {message.text}
        </span>
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
  const [detail, setDetail] = useState('')
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
          detail: detail || null,
          dueAt: dueAt || null,
          personId,
          propertyId,
          dealId,
        }),
      )
      if (result.ok) {
        setTitle('')
        setDetail('')
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
            Due (optional, local time)
          </span>
          <input
            type="datetime-local"
            value={dueAt}
            onChange={(event) => setDueAt(event.target.value)}
            className="mt-2 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          />
        </label>
      </div>
      <label className="block">
        <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
          Detail (optional)
        </span>
        <input
          value={detail}
          onChange={(event) => setDetail(event.target.value)}
          placeholder="Context for this follow-up"
          className="mt-2 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
        />
      </label>
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
              message.ok ? 'text-black/50' : 'text-[var(--portal-archive)]'
            }`}
          >
            {message.text}
          </span>
        )}
      </div>
    </form>
  )
}
