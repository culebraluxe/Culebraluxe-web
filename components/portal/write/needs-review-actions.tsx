'use client'

import { useState, useTransition } from 'react'

import { resolveIntakeAction } from '@/app/portal/actions'
import type { ResolveIntakeAction } from '@/db/needs-review-resolution'
import { PersonSelector } from '@/components/portal/write/person-selector'

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
  const [personLabel, setPersonLabel] = useState<string | null>(null)
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )

  function submit(action: ResolveIntakeAction, successText: string) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await resolveIntakeAction({ submissionId, action }),
      )
      if (result.ok) {
        setPersonId('')
        setPersonLabel(null)
        setMessage({ ok: true, text: successText })
      } else {
        setMessage({ ok: false, text: result.message ?? 'Action failed.' })
      }
    })
  }

  function attach(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!personId) {
      setMessage({ ok: false, text: 'Select an existing person first.' })
      return
    }
    submit({ kind: 'attach', personId }, 'Attached to person.')
  }

  return (
    <div className="space-y-3 border-t border-[var(--portal-border)] pt-4">
      <form onSubmit={attach} className="space-y-3">
        <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
          <div>
            <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
              Attach to existing person
            </span>
            <div className="mt-2">
              <PersonSelector
                selectedLabel={personLabel}
                onSelect={(id, label) => {
                  setPersonId(id)
                  setPersonLabel(label || null)
                }}
                placeholder="Search existing people…"
              />
            </div>
          </div>
          <div className="self-end">
            <button
              type="submit"
              disabled={isPending || !personId}
              className={secondaryButton}
            >
              Attach
            </button>
          </div>
        </div>
      </form>

      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          disabled={isPending}
          onClick={() =>
            submit({ kind: 'create' }, 'Person created and intake completed.')
          }
          className={secondaryButton}
        >
          Create person &amp; complete
        </button>

        <button
          type="button"
          disabled={isPending}
          onClick={() => submit({ kind: 'reject' }, 'Rejected.')}
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
