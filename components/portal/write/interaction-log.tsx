'use client'

import { useState, useTransition } from 'react'

import { logManualInteractionAction } from '@/app/portal/actions'

export function InteractionLogForm({
  personId,
  propertyId,
  dealId,
}: {
  personId: string
  propertyId?: string
  dealId?: string
}) {
  const [isPending, startTransition] = useTransition()
  const [channel, setChannel] = useState<'manual' | 'note'>('note')
  const [title, setTitle] = useState('')
  const [summary, setSummary] = useState('')
  const [occurredAt, setOccurredAt] = useState('')
  const [message, setMessage] = useState<{
    ok: boolean
    text: string
  } | null>(null)

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = (await logManualInteractionAction({
        personId,
        channel,
        title: title || undefined,
        summary: summary || undefined,
        occurredAt: occurredAt || undefined,
        propertyId,
        dealId,
      })) as { ok: boolean; message?: string }
      if (result.ok) {
        setTitle('')
        setSummary('')
        setOccurredAt('')
        setMessage({ ok: true, text: 'Logged to the timeline.' })
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not log.' })
      }
    })
  }

  return (
    <form onSubmit={submit} className="space-y-3">
      <div className="flex flex-wrap items-center gap-3">
        <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
          Channel
        </span>
        <label className="flex items-center gap-1.5 text-xs font-light text-black/60">
          <input
            type="radio"
            name="channel"
            checked={channel === 'note'}
            onChange={() => setChannel('note')}
            className="accent-[var(--portal-navy)]"
          />
          Note
        </label>
        <label className="flex items-center gap-1.5 text-xs font-light text-black/60">
          <input
            type="radio"
            name="channel"
            checked={channel === 'manual'}
            onChange={() => setChannel('manual')}
            className="accent-[var(--portal-navy)]"
          />
          Manual entry
        </label>
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Title (optional)
          </span>
          <input
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="Phone call summary"
            className="mt-2 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          />
        </label>
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Date (optional)
          </span>
          <input
            type="datetime-local"
            value={occurredAt}
            onChange={(event) => setOccurredAt(event.target.value)}
            className="mt-2 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          />
        </label>
      </div>

      <label className="block">
        <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
          Details
        </span>
        <textarea
          value={summary}
          onChange={(event) => setSummary(event.target.value)}
          rows={3}
          placeholder="What happened in this interaction?"
          className="mt-2 block w-full resize-y rounded-sm border border-[var(--portal-border)] bg-white px-3 py-2.5 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
        />
      </label>

      <div className="flex items-center gap-3">
        <button
          type="submit"
          disabled={isPending}
          className="inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-4 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"
        >
          Log to timeline
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
