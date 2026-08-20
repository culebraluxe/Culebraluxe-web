'use client'

import { useState, useTransition } from 'react'

import {
  attachIntakeToPersonAction,
  rejectIntakeAction,
} from '@/app/portal/actions'

type Result = { ok: boolean; message?: string }

function resolve(result: unknown): Result {
  return result as Result
}

const secondaryButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40'

const ghostButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm px-3 text-[11px] font-light uppercase tracking-[0.14em] text-black/45 transition hover:text-[#8a4b2a] disabled:cursor-not-allowed disabled:opacity-40'

export function NeedsReviewActions({
  submissionId,
}: {
  submissionId: string
}) {
  const [isPending, startTransition] = useTransition()
  const [personId, setPersonId] = useState('')
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )

  function run(action: () => Promise<unknown>) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await action())
      if (!result.ok) {
        setMessage({ ok: false, text: result.message ?? 'Action failed.' })
      }
    })
  }

  function attach(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!personId.trim()) return
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await attachIntakeToPersonAction(submissionId, personId.trim()),
      )
      if (result.ok) {
        setPersonId('')
        setMessage({ ok: true, text: 'Attached to person.' })
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not attach.' })
      }
    })
  }

  return (
    <div className="mt-4 space-y-3 border-t border-[var(--portal-border)] pt-4">
      <form onSubmit={attach} className="flex flex-wrap items-end gap-3">
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Attach to existing person (person id)
          </span>
          <input
            value={personId}
            onChange={(event) => setPersonId(event.target.value)}
            placeholder="00000000-0000-0000-0000-000000000000"
            className="mt-2 block min-h-11 w-80 rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          />
        </label>
        <button
          type="submit"
          disabled={isPending || !personId.trim()}
          className={secondaryButton}
        >
          Attach
        </button>
      </form>

      <div className="flex items-center gap-3">
        <button
          type="button"
          disabled={isPending}
          onClick={() => run(() => rejectIntakeAction(submissionId))}
          className={ghostButton}
        >
          Reject & close
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
    </div>
  )
}
