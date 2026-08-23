"use client"

import { useRouter } from "next/navigation"
import { useState, useTransition } from "react"

import { createDealAction } from "@/app/portal/actions"
import { PersonSelector } from "@/components/portal/write/person-selector"
import type { DealableProperty } from "@/db/deals"

// OPS-05 — "New deal": create a canonical deal record from the Deals
// Portfolio index. A deal always belongs to an existing active property and
// always has a client person (deal.client_person_id is NOT NULL); the owner is
// an optional internal app user. The canonical client/owner participant rows
// are created with the deal (participant-model decision, migration 034), so
// the participants list is correct from birth. Stage/date/financing stay
// workflow-owned and are not part of this panel.
export function DealCreatePanel({
  properties,
  users,
}: {
  properties: DealableProperty[]
  users: { id: string; displayName: string; email: string | null }[]
}) {
  const router = useRouter()
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )
  const [open, setOpen] = useState(false)

  const [propertyId, setPropertyId] = useState("")
  const [clientPersonId, setClientPersonId] = useState("")
  const [clientLabel, setClientLabel] = useState<string | null>(null)
  const [ownerUserId, setOwnerUserId] = useState("")
  const [notes, setNotes] = useState("")

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!propertyId) {
      setMessage({ ok: false, text: "Choose a property first." })
      return
    }
    if (!clientPersonId) {
      setMessage({ ok: false, text: "Select an existing client person first." })
      return
    }
    setMessage(null)
    startTransition(async () => {
      const result = await createDealAction({
        propertyId,
        clientPersonId,
        ownerUserId: ownerUserId || null,
        notes: notes.trim() || null,
      })
      if (!result.ok) {
        setMessage({ ok: false, text: result.message })
        return
      }
      setPropertyId("")
      setClientPersonId("")
      setClientLabel(null)
      setOwnerUserId("")
      setNotes("")
      setOpen(false)
      router.push(`/portal/deals/${result.data.id}`)
      router.refresh()
    })
  }

  const inputClass =
    "mt-1 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
  const labelClass =
    "mb-1 block text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]"

  return (
    <section className="portal-glass-panel mb-6 overflow-hidden rounded-[var(--portal-panel-radius)]">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--portal-border)] px-6 py-5">
        <div>
          <h2 className="font-serif text-2xl font-light">New deal</h2>
          <p className="mt-1 text-xs font-light text-black/40">
            Open a deal against an active property and its client; the canonical
            participant rows are created with it.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setOpen(!open)}
          className="inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-5 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)]"
        >
          {open ? "Close" : "New deal"}
        </button>
      </div>

      {open && (
        <form onSubmit={submit} className="space-y-4 p-6">
          <div className="grid gap-4 md:grid-cols-2">
            <label className="block">
              <span className={labelClass}>Property *</span>
              <select
                className={inputClass}
                value={propertyId}
                onChange={(event) => setPropertyId(event.target.value)}
              >
                <option value="">Choose an active property…</option>
                {properties.map((property) => (
                  <option key={property.id} value={property.id}>
                    {property.name}
                    {property.location ? ` — ${property.location}` : ""}
                  </option>
                ))}
              </select>
            </label>

            <div>
              <span className={labelClass}>Client person *</span>
              <div className="mt-1">
                <PersonSelector
                  selectedLabel={clientLabel}
                  onSelect={(id, label) => {
                    setClientPersonId(id)
                    setClientLabel(label || null)
                  }}
                  placeholder="Search existing people…"
                />
              </div>
            </div>

            <label className="block">
              <span className={labelClass}>Owner (internal agent)</span>
              <select
                className={inputClass}
                value={ownerUserId}
                onChange={(event) => setOwnerUserId(event.target.value)}
              >
                <option value="">Unassigned…</option>
                {users.map((user) => (
                  <option key={user.id} value={user.id}>
                    {user.displayName}
                    {user.email ? ` — ${user.email}` : ""}
                  </option>
                ))}
              </select>
            </label>

            <label className="block">
              <span className={labelClass}>Notes</span>
              <textarea
                className={`${inputClass} min-h-24 resize-y py-2`}
                value={notes}
                onChange={(event) => setNotes(event.target.value)}
                placeholder="Optional deal notes…"
              />
            </label>
          </div>

          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={isPending}
              className="inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-5 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"
            >
              Create deal
            </button>
            {message && (
              <span
                className={`text-xs font-light ${
                  message.ok ? "text-black/50" : "text-[var(--portal-archive)]"
                }`}
              >
                {message.text}
              </span>
            )}
          </div>
        </form>
      )}
    </section>
  )
}
