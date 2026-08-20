'use client'

import { useState, useTransition } from 'react'

import {
  updatePersonNotesAction,
  updatePersonStatusAction,
} from '@/app/portal/actions'

type Result = { ok: boolean; message?: string }

function resolve(result: unknown): Result {
  return result as Result
}

const STATUSES = [
  { value: 'new', label: 'New' },
  { value: 'warm', label: 'Warm' },
  { value: 'active', label: 'Active' },
  { value: 'referral', label: 'Referral' },
]

const primaryButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40'

export function PersonActions({
  personId,
  initialStatus,
  initialNotes,
}: {
  personId: string
  initialStatus: string
  initialNotes: string | null
}) {
  const [isPending, startTransition] = useTransition()
  const [status, setStatus] = useState(initialStatus)
  const [notes, setNotes] = useState(initialNotes ?? '')
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )

  function saveStatus(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await updatePersonStatusAction(personId, status),
      )
      if (result.ok) {
        setMessage({ ok: true, text: 'Status saved.' })
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not save.' })
      }
    })
  }

  function saveNotes(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await updatePersonNotesAction(personId, notes.trim() || null),
      )
      if (result.ok) {
        setMessage({ ok: true, text: 'Notes saved.' })
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not save.' })
      }
    })
  }

  return (
    <div className="space-y-4">
      <form onSubmit={saveStatus} className="flex flex-wrap items-end gap-3">
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Relationship status
          </span>
          <select
            value={status}
            onChange={(event) => setStatus(event.target.value)}
            className="mt-2 block min-h-11 w-44 rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          >
            {STATUSES.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <button
          type="submit"
          disabled={isPending}
          className={primaryButton}
        >
          Save status
        </button>
      </form>

      <form onSubmit={saveNotes} className="space-y-3">
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Client notes
          </span>
          <textarea
            value={notes}
            onChange={(event) => setNotes(event.target.value)}
            rows={4}
            placeholder="Preferences, context, follow-ups…"
            className="mt-2 block w-full resize-y rounded-sm border border-[var(--portal-border)] bg-white px-3 py-2.5 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          />
        </label>
        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={isPending}
            className={primaryButton}
          >
            Save notes
          </button>
        </div>
      </form>

      {message && (
        <p
          className={`text-xs font-light ${
            message.ok ? 'text-black/50' : 'text-[#8a4b2a]'
          }`}
        >
          {message.text}
        </p>
      )}
    </div>
  )
}
