'use client'

import { useState, useTransition } from 'react'

import {
  cancelShowingAction,
  completeShowingAction,
  createShowingAction,
  scheduleShowingAction,
} from '@/app/portal/actions'
import { PersonSelector } from '@/components/portal/write/person-selector'

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

const fieldInput =
  'mt-2 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]'

export function ShowingActions({
  showingId,
  status,
}: {
  showingId: string
  status: string
}) {
  const [isPending, startTransition] = useTransition()
  const [scheduledAt, setScheduledAt] = useState('')
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

  const canSchedule = status === 'requested'
  const canComplete = status === 'requested' || status === 'scheduled'
  const canCancel = status === 'requested' || status === 'scheduled'

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-2">
        {canSchedule && (
          <>
            <input
              type="datetime-local"
              value={scheduledAt}
              onChange={(event) => setScheduledAt(event.target.value)}
              aria-label="Schedule date and time (local)"
              className="block min-h-11 w-52 rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
            />
            <button
              type="button"
              disabled={isPending || !scheduledAt}
              onClick={() =>
                run(() => scheduleShowingAction(showingId, scheduledAt))
              }
              className={primaryButton}
            >
              Schedule
            </button>
          </>
        )}
        {canComplete && (
          <button
            type="button"
            disabled={isPending}
            onClick={() => run(() => completeShowingAction(showingId))}
            className={secondaryButton}
          >
            Complete
          </button>
        )}
        {canCancel && (
          <button
            type="button"
            disabled={isPending}
            onClick={() => run(() => cancelShowingAction(showingId))}
            className={ghostButton}
          >
            Cancel
          </button>
        )}
      </div>
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
  )
}

export function CreateShowingForm({
  personId,
  propertyId,
  dealId,
}: {
  personId: string
  propertyId?: string
  dealId?: string
}) {
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await createShowingAction({
          personId,
          propertyId,
          dealId,
        }),
      )
      if (result.ok) {
        setMessage({ ok: true, text: 'Showing requested.' })
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not request.' })
      }
    })
  }

  return (
    <form onSubmit={submit} className="flex flex-wrap items-center gap-3">
      <button
        type="submit"
        disabled={isPending}
        className={primaryButton}
      >
        Request showing
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

export type ShowingPropertyOption = {
  id: string
  name: string
  location: string | null
}

export function CreateShowingPanel({
  properties,
}: {
  properties: ShowingPropertyOption[]
}) {
  const [isPending, startTransition] = useTransition()
  const [personId, setPersonId] = useState('')
  const [personLabel, setPersonLabel] = useState<string | null>(null)
  const [propertyId, setPropertyId] = useState('')
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!personId) {
      setMessage({ ok: false, text: 'Select a person first.' })
      return
    }
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await createShowingAction({
          personId,
          propertyId: propertyId || undefined,
        }),
      )
      if (result.ok) {
        setPersonId('')
        setPersonLabel(null)
        setPropertyId('')
        setMessage({ ok: true, text: 'Showing requested.' })
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not request.' })
      }
    })
  }

  return (
    <form onSubmit={submit} className="space-y-3">
      <div className="grid gap-3 md:grid-cols-2">
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Person
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
        </label>
        <label className="block">
          <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            Property (optional)
          </span>
          <select
            value={propertyId}
            onChange={(event) => setPropertyId(event.target.value)}
            className={fieldInput}
          >
            <option value="">Select a property</option>
            {properties.map((property) => (
              <option key={property.id} value={property.id}>
                {property.name}
                {property.location ? ` · ${property.location}` : ''}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="flex items-center gap-3">
        <button
          type="submit"
          disabled={isPending || !personId}
          className={primaryButton}
        >
          Request showing
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
