'use client'

import { useState, useTransition } from 'react'

import {
  rejectOfferAction,
  submitOfferAction,
  withdrawOfferAction,
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

export function OfferActions({
  offerId,
  status,
}: {
  offerId: string
  status: string
}) {
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<string | null>(null)

  function run(action: () => Promise<unknown>) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await action())
      if (!result.ok) setMessage(result.message ?? 'Action failed.')
    })
  }

  const isSubmitted = status === 'submitted'

  if (!isSubmitted) return null

  return (
    <div className="flex flex-wrap items-center gap-2">
      <button
        type="button"
        disabled={isPending}
        onClick={() => run(() => withdrawOfferAction(offerId))}
        className={secondaryButton}
      >
        Withdraw
      </button>
      <button
        type="button"
        disabled={isPending}
        onClick={() => run(() => rejectOfferAction(offerId))}
        className={ghostButton}
      >
        Reject
      </button>
      {message && (
        <span className="text-xs font-light text-[#8a4b2a]">{message}</span>
      )}
    </div>
  )
}

export function OfferForm({
  dealId,
  personId,
  parentOfferId,
  label = 'Submit offer',
}: {
  dealId: string
  personId: string
  parentOfferId?: string
  label?: string
}) {
  const [isPending, startTransition] = useTransition()
  const [amount, setAmount] = useState('')
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const parsed = Number(amount)
    if (!Number.isFinite(parsed) || parsed <= 0) {
      setMessage({ ok: false, text: 'Enter a valid positive amount.' })
      return
    }
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await submitOfferAction({
          dealId,
          personId,
          amount: parsed,
          parentOfferId,
        }),
      )
      if (result.ok) {
        setAmount('')
        setMessage({ ok: true, text: 'Offer submitted.' })
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not submit.' })
      }
    })
  }

  return (
    <form onSubmit={submit} className="flex flex-wrap items-end gap-3">
      <label className="block">
        <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
          Amount (USD)
        </span>
        <input
          type="number"
          min="0"
          step="any"
          value={amount}
          onChange={(event) => setAmount(event.target.value)}
          required
          placeholder="0"
          className="mt-2 block min-h-11 w-40 rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
        />
      </label>
      <button
        type="submit"
        disabled={isPending}
        className={primaryButton}
      >
        {label}
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
    </form>
  )
}
