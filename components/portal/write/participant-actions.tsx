'use client'

import { useState, useTransition } from 'react'

import {
  addOtherParticipantAction,
  endParticipantAction,
  updateParticipantRoleLabelAction,
} from '@/app/portal/actions'

type Result = { ok: boolean; message?: string }

function resolve(result: unknown): Result {
  return result as Result
}

const primaryButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40'

const secondaryButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40'

const ghostButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm px-3 text-[11px] font-light uppercase tracking-[0.14em] text-black/45 transition hover:text-[#8a4b2a] disabled:cursor-not-allowed disabled:opacity-40'

export function AddOtherParticipantForm({ dealId }: { dealId: string }) {
  const [isPending, startTransition] = useTransition()
  const [personId, setPersonId] = useState('')
  const [roleLabel, setRoleLabel] = useState('')
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!roleLabel.trim()) return
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await addOtherParticipantAction({
          dealId,
          personId: personId.trim() || undefined,
          roleLabel,
        }),
      )
      if (result.ok) {
        setPersonId('')
        setRoleLabel('')
        setMessage({ ok: true, text: 'Participant added.' })
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not add.' })
      }
    })
  }

  return (
    <form
      onSubmit={submit}
      className="space-y-3 border-t border-[var(--portal-border)] px-6 py-5"
    >
      <p className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
        Add other participant
      </p>
      <div className="grid gap-3 md:grid-cols-2">
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Role label (e.g. Attorney, Lender)
          </span>
          <input
            value={roleLabel}
            onChange={(event) => setRoleLabel(event.target.value)}
            required
            placeholder="Attorney"
            className="mt-2 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          />
        </label>
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Person id (optional)
          </span>
          <input
            value={personId}
            onChange={(event) => setPersonId(event.target.value)}
            placeholder="00000000-0000-0000-0000-000000000000"
            className="mt-2 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          />
        </label>
      </div>
      <div className="flex items-center gap-3">
        <button
          type="submit"
          disabled={isPending}
          className={primaryButton}
        >
          Add participant
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

export function OtherParticipantActions({
  participantId,
  roleCategory,
}: {
  participantId: string
  roleCategory: string
}) {
  const [isPending, startTransition] = useTransition()
  const [roleLabel, setRoleLabel] = useState('')
  const [message, setMessage] = useState<string | null>(null)

  if (roleCategory !== 'other') return null

  function run(action: () => Promise<unknown>) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await action())
      if (!result.ok) setMessage(result.message ?? 'Action failed.')
    })
  }

  function updateRole(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!roleLabel.trim()) return
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await updateParticipantRoleLabelAction(participantId, roleLabel.trim()),
      )
      if (result.ok) setRoleLabel('')
      else setMessage(result.message ?? 'Could not update.')
    })
  }

  return (
    <div className="mt-2 flex flex-wrap items-center gap-2">
      <form onSubmit={updateRole} className="flex items-center gap-2">
        <input
          value={roleLabel}
          onChange={(event) => setRoleLabel(event.target.value)}
          placeholder="New role label"
          aria-label="New role label"
          className="block min-h-11 w-44 rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
        />
        <button
          type="submit"
          disabled={isPending}
          className={secondaryButton}
        >
          Update
        </button>
      </form>
      <button
        type="button"
        disabled={isPending}
        onClick={() => run(() => endParticipantAction(participantId))}
        className={ghostButton}
      >
        End
      </button>
      {message && (
        <span className="text-xs font-light text-[#8a4b2a]">{message}</span>
      )}
    </div>
  )
}
