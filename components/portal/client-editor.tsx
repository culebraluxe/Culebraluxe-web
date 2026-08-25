"use client"

import { useState, useTransition } from "react"
import type { Client } from "@/lib/portal/types"
import {
  createClientAction,
  updateClientIdentityAction,
  updateClientProfileAction,
} from "@/app/portal/actions"
import type {
  ClientCreateInput,
  ClientProfileFields,
} from "@/lib/person-admin"

export type ClientEditorAgent = { id: string; displayName: string }

function listToCsv(values: string[] | undefined) {
  return values?.join(", ") ?? ""
}

function csvToList(value: string) {
  return value
    .split(",")
    .map((part) => part.trim())
    .filter((part) => part.length > 0)
}

function numberOrNull(value: string) {
  if (!value.trim()) return null
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : null
}

const ROLES = [
  { value: "buyer", label: "Buyer" },
  { value: "seller", label: "Seller" },
  { value: "both", label: "Buyer & Seller" },
]

const STATUSES = [
  { value: "new", label: "New" },
  { value: "warm", label: "Warm" },
  { value: "active", label: "Active" },
  { value: "referral", label: "Referral" },
]

const inputClass =
  "mt-2 block w-full rounded-[var(--portal-panel-radius)] portal-glass-panel px-3 py-2.5 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"

const labelClass =
  "text-[10px] font-light uppercase tracking-[0.18em] text-black/40"

function Field({
  label,
  children,
  span = false,
}: {
  label: string
  children: React.ReactNode
  span?: boolean
}) {
  return (
    <label className={`block ${span ? "md:col-span-2" : ""}`}>
      <span className={labelClass}>{label}</span>
      {children}
    </label>
  )
}

export function ClientEditor({
  mode,
  client,
  agents,
  onSaved,
  onCancel,
}: {
  mode: "create" | "edit"
  client?: Client
  agents: ClientEditorAgent[]
  onSaved?: (personId: string) => void
  onCancel?: () => void
}) {
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )

  const [displayName, setDisplayName] = useState(client?.displayName ?? "")
  const [role, setRole] = useState<string>(client?.role ?? "buyer")
  const [status, setStatus] = useState<string>(client?.status ?? "new")
  const [location, setLocation] = useState(client?.location ?? "")
  const [email, setEmail] = useState(client?.email ?? "")
  const [phone, setPhone] = useState(client?.phone ?? "")
  const [budgetMin, setBudgetMin] = useState(
    client?.budgetMin != null ? String(client.budgetMin) : "",
  )
  const [budgetMax, setBudgetMax] = useState(
    client?.budgetMax != null ? String(client.budgetMax) : "",
  )
  const [preferredAreas, setPreferredAreas] = useState(
    listToCsv(client?.preferredAreas),
  )
  const [propertyTypes, setPropertyTypes] = useState(
    listToCsv(client?.propertyTypes),
  )
  const [priorities, setPriorities] = useState(listToCsv(client?.priorities))
  const [timeline, setTimeline] = useState(client?.timeline ?? "")
  const [notes, setNotes] = useState(client?.notes ?? "")
  const [assignedUserId, setAssignedUserId] = useState(
    client?.assignedUserId ?? "",
  )

  function profileInput(): ClientProfileFields & { displayName: string } {
    const min = numberOrNull(budgetMin)
    const max = numberOrNull(budgetMax)
    return {
      displayName: displayName.trim(),
      role: role as ClientProfileFields["role"],
      status: status as ClientProfileFields["status"],
      location: location.trim() || null,
      budgetMin: min,
      budgetMax: max,
      preferredAreas: csvToList(preferredAreas),
      propertyTypes: csvToList(propertyTypes),
      priorities: csvToList(priorities),
      timeline: timeline.trim() || null,
      notes: notes.trim() || null,
      assignedUserId: assignedUserId || null,
    }
  }

  function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)

    startTransition(async () => {
      if (mode === "create") {
        const input: ClientCreateInput = {
          ...profileInput(),
          role: role as ClientCreateInput["role"],
          email: email.trim() || null,
          phone: phone.trim() || null,
        }
        const result = await createClientAction(input)
        if (!result.ok) {
          setMessage({ ok: false, text: result.message })
          return
        }
        setMessage({ ok: true, text: "Client created." })
        onSaved?.(result.data.personId)
        return
      }

      if (!client) return

      const profile = await updateClientProfileAction(client.id, profileInput())
      if (!profile.ok) {
        setMessage({ ok: false, text: profile.message })
        return
      }

      // Contact identities are separate rows; update only when changed.
      if (email.trim() !== (client.email ?? "")) {
        const identity = await updateClientIdentityAction(
          client.id,
          "email",
          email.trim() || null,
        )
        if (!identity.ok) {
          setMessage({
            ok: false,
            text: `Profile saved, but email was not: ${identity.message}`,
          })
          onSaved?.(client.id)
          return
        }
      }
      if (phone.trim() !== (client.phone ?? "")) {
        const identity = await updateClientIdentityAction(
          client.id,
          "phone",
          phone.trim() || null,
        )
        if (!identity.ok) {
          setMessage({
            ok: false,
            text: `Profile saved, but phone was not: ${identity.message}`,
          })
          onSaved?.(client.id)
          return
        }
      }

      setMessage({ ok: true, text: "Client profile saved." })
      onSaved?.(client.id)
    })
  }

  return (
    <form
      onSubmit={save}
      className="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6 lg:p-8"
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="font-serif text-2xl font-light">
            {mode === "create" ? "New Client" : `Edit ${client?.displayName}`}
          </h3>
          <p className="mt-1 text-xs font-light text-black/40">
            {mode === "create"
              ? "Add a buyer or seller to the client roster."
              : "Update profile details and contact information."}
          </p>
        </div>

        <div className="flex items-center gap-3">
          {onCancel && (
            <button
              type="button"
              onClick={onCancel}
              className="rounded-full border border-[var(--portal-border)] px-4 py-2 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)]"
            >
              Cancel
            </button>
          )}

          <button
            type="submit"
            disabled={isPending}
            className="inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-5 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"
          >
            {isPending
              ? "Saving…"
              : mode === "create"
                ? "Create client"
                : "Save changes"}
          </button>
        </div>
      </div>

      <div className="mt-8 grid gap-5 md:grid-cols-2">
        <Field label="Display name *">
          <input
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            required
            className={inputClass}
          />
        </Field>

        <div className="grid grid-cols-2 gap-4">
          <Field label="Role *">
            <select
              value={role}
              onChange={(event) => setRole(event.target.value)}
              className={inputClass}
            >
              {ROLES.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </Field>

          <Field label="Status">
            <select
              value={status}
              onChange={(event) => setStatus(event.target.value)}
              className={inputClass}
            >
              {STATUSES.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </Field>
        </div>

        <Field label="Location">
          <input
            value={location}
            onChange={(event) => setLocation(event.target.value)}
            placeholder="Culebra, Puerto Rico"
            className={inputClass}
          />
        </Field>

        <Field label="Assigned agent">
          <select
            value={assignedUserId}
            onChange={(event) => setAssignedUserId(event.target.value)}
            className={inputClass}
          >
            <option value="">Unassigned</option>
            {agents.map((agent) => (
              <option key={agent.id} value={agent.id}>
                {agent.displayName}
              </option>
            ))}
          </select>
        </Field>

        <Field label="Email">
          <input
            type="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            className={inputClass}
          />
        </Field>

        <Field label="Phone (E.164)">
          <input
            value={phone}
            onChange={(event) => setPhone(event.target.value)}
            placeholder="+1787…"
            className={inputClass}
          />
        </Field>

        <Field label="Budget minimum">
          <input
            type="number"
            min="0"
            step="any"
            value={budgetMin}
            onChange={(event) => setBudgetMin(event.target.value)}
            className={inputClass}
          />
        </Field>

        <Field label="Budget maximum">
          <input
            type="number"
            min="0"
            step="any"
            value={budgetMax}
            onChange={(event) => setBudgetMax(event.target.value)}
            className={inputClass}
          />
        </Field>

        <Field label="Preferred areas" span>
          <input
            value={preferredAreas}
            onChange={(event) => setPreferredAreas(event.target.value)}
            placeholder="Culebra town, Flamenco, …"
            className={inputClass}
          />
        </Field>

        <Field label="Property types" span>
          <input
            value={propertyTypes}
            onChange={(event) => setPropertyTypes(event.target.value)}
            placeholder="Villa, Condo, …"
            className={inputClass}
          />
        </Field>

        <Field label="Priorities" span>
          <input
            value={priorities}
            onChange={(event) => setPriorities(event.target.value)}
            placeholder="Ocean view, privacy, …"
            className={inputClass}
          />
        </Field>

        <Field label="Timeline" span>
          <input
            value={timeline}
            onChange={(event) => setTimeline(event.target.value)}
            placeholder="Looking within 6 months"
            className={inputClass}
          />
        </Field>

        <Field label="Notes" span>
          <textarea
            value={notes}
            onChange={(event) => setNotes(event.target.value)}
            rows={4}
            placeholder="Preferences, context, follow-ups…"
            className={`${inputClass} resize-y`}
          />
        </Field>
      </div>

      {message && (
        <p
          className={`mt-6 text-xs font-light ${
            message.ok ? "text-black/50" : "text-[var(--portal-archive)]"
          }`}
        >
          {message.text}
        </p>
      )}
    </form>
  )
}
