"use client"

import Link from "next/link"
import { useEffect, useMemo, useState } from "react"
import type {
  Client,
  ClientStatus,
  InteractionChannel,
  PropertyInterestStatus,
} from "@/lib/portal/types"
import { Panel } from "@/components/portal/panel"
import { ClientEditor } from "@/components/portal/client-editor"
import type { ClientEditorAgent } from "@/components/portal/client-editor"
import { InteractionLogForm } from "@/components/portal/write/interaction-log"
import { ClientDailyActions } from "@/components/portal/client-daily-actions"

function formatCurrency(value?: number) {
  if (!value) return "—"

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(value)
}

function roleLabel(role: Client["role"]) {
  if (role === "both") return "Buyer & Seller"
  return role.charAt(0).toUpperCase() + role.slice(1)
}

function statusLabel(status: ClientStatus) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

function interestStatusLabel(status: PropertyInterestStatus) {
  switch (status) {
    case "tour_completed":
      return "Tour completed"
    case "shortlisted":
      return "Shortlisted"
    default:
      return "Interested"
  }
}

function channelLabel(channel: InteractionChannel) {
  switch (channel) {
    case "website":
      return "Website"
    case "calendar":
      return "Calendar"
    case "document":
      return "Document"
    case "manual":
      return "Manual entry"
    case "imessage":
      return "iMessage"
    case "sms":
      return "SMS"
    case "email":
      return "Email"
    case "call":
      return "Phone call"
    case "meeting":
      return "Meeting"
    case "showing":
      return "Showing"
    case "whatsapp":
      return "WhatsApp"
    default:
      return "Note"
  }
}

function initials(name: string) {
  return name
    .split(" ")
    .slice(0, 2)
    .map((word) => word[0])
    .join("")
}

const ghostBtn =
  "inline-flex min-h-9 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]"

export function ClientManager({
  clients,
  agents,
}: {
  clients: Client[]
  agents: ClientEditorAgent[]
}) {
  const [query, setQuery] = useState("")
  const [selectedClientId, setSelectedClientId] = useState(
    clients[0]?.id ?? ""
  )
  const [showCreate, setShowCreate] = useState(false)
  const [showEdit, setShowEdit] = useState(false)

  const filteredClients = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    if (!normalized) return clients

    return clients.filter((client) =>
      [
        client.displayName,
        client.location,
        client.email,
        client.phone,
        client.role,
        client.status,
        client.nextAction?.title,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
        .includes(normalized)
    )
  }, [clients, query])

  const selectedClient =
    clients.find((client) => client.id === selectedClientId) ??
    filteredClients[0] ??
    clients[0]

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const target = event.target
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement
      ) {
        return
      }
      const ids = filteredClients.map((client) => client.id)
      const index = selectedClient ? ids.indexOf(selectedClient.id) : -1
      if (event.key === "ArrowDown" && index >= 0 && index < ids.length - 1) {
        event.preventDefault()
        setSelectedClientId(ids[index + 1])
      }
      if (event.key === "ArrowUp" && index > 0) {
        event.preventDefault()
        setSelectedClientId(ids[index - 1])
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [filteredClients, selectedClient])

  return (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="grid min-h-0 flex-1 gap-4 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[200px_minmax(0,1fr)]">
        <aside className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
            <div className="mb-2 flex items-center justify-between gap-2">
              <span className="text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                People · {filteredClients.length}
              </span>
              <button
                type="button"
                onClick={() => {
                  setShowCreate(true)
                  setShowEdit(false)
                }}
                className="inline-flex min-h-7 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white transition hover:bg-[var(--portal-navy-soft)]"
              >
                New
              </button>
            </div>
            <input
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search…"
              className="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
            />
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto">
            {filteredClients.length === 0 ? (
              <p className="px-3 py-6 text-sm font-light text-black/40">
                No matching clients.
              </p>
            ) : (
              filteredClients.map((client) => {
                const selected = selectedClient?.id === client.id
                return (
                  <button
                    type="button"
                    key={client.id}
                    onClick={() => {
                      setSelectedClientId(client.id)
                      setShowCreate(false)
                      setShowEdit(false)
                    }}
                    ref={
                      selected
                        ? (node) => node?.scrollIntoView({ block: "nearest" })
                        : undefined
                    }
                    className={[
                      "flex w-full items-center gap-2 border-b border-[var(--portal-panel-border)] px-2.5 py-2 text-left transition",
                      selected
                        ? "border-l-2 border-l-[var(--portal-gold)] bg-white/40"
                        : "border-l-2 border-l-transparent hover:bg-white/25",
                    ].join(" ")}
                  >
                    <span
                      className={`h-1.5 w-1.5 shrink-0 rounded-full ${
                        client.status === "active"
                          ? "bg-[var(--portal-success)]"
                          : client.status === "warm"
                            ? "bg-[var(--portal-navy-soft)]"
                            : client.status === "referral"
                              ? "bg-[var(--portal-neutral)]"
                              : "bg-black/25"
                      }`}
                      aria-label={statusLabel(client.status)}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-medium text-[var(--portal-navy)]">
                        {client.displayName}
                      </div>
                      <div className="truncate text-[11px] font-light text-black/45">
                        {client.nextAction?.title ?? roleLabel(client.role)}
                      </div>
                    </div>
                  </button>
                )
              })
            )}
          </div>
        </aside>

        <div className="min-h-0 overflow-y-auto">
          {showCreate ? (
            <Panel
              compact
              heading="New client"
              action={
                <button
                  type="button"
                  onClick={() => setShowCreate(false)}
                  className={ghostBtn}
                >
                  Cancel
                </button>
              }
            >
              <ClientEditor
                mode="create"
                agents={agents}
                onSaved={(personId) => {
                  setSelectedClientId(personId)
                  setShowCreate(false)
                }}
                onCancel={() => setShowCreate(false)}
              />
            </Panel>
          ) : !selectedClient ? (
            <Panel compact heading="No clients yet">
              <p className="text-sm font-light text-black/45">
                Add a client to start a relationship file.
              </p>
            </Panel>
          ) : (
            <ClientPane
              client={selectedClient}
              agents={agents}
              showEdit={showEdit}
              setShowEdit={setShowEdit}
            />
          )}
        </div>
      </div>
    </div>
  )
}

function ClientPane({
  client,
  agents,
  showEdit,
  setShowEdit,
}: {
  client: Client
  agents: ClientEditorAgent[]
  showEdit: boolean
  setShowEdit: (value: boolean) => void
}) {
  return (
    <div className="flex flex-col gap-3">
      <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] font-serif text-base font-light text-[var(--portal-navy-soft)]">
              {initials(client.displayName)}
            </div>
            <div className="min-w-0">
              <h2 className="font-serif text-2xl font-light leading-tight text-[var(--portal-navy)]">
                {client.displayName}
              </h2>
              <p className="mt-0.5 truncate text-xs font-light text-black/45">
                {roleLabel(client.role)}
                {client.location ? ` · ${client.location}` : ""}
                {client.email ? ` · ${client.email}` : ""}
                {client.phone ? ` · ${client.phone}` : ""}
              </p>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Link href={`/portal/clients/${client.id}`} className={ghostBtn}>
              Dossier
            </Link>
            <button
              type="button"
              onClick={() => setShowEdit(!showEdit)}
              className={ghostBtn}
            >
              {showEdit ? "Close" : "Edit"}
            </button>
          </div>
        </div>

        <div className="mt-4 grid gap-3 border-t border-[var(--portal-panel-border)] pt-3 sm:grid-cols-2 lg:grid-cols-4">
          <Detail
            label="Budget"
            value={
              client.budgetMin || client.budgetMax
                ? `${formatCurrency(client.budgetMin)} – ${formatCurrency(
                    client.budgetMax
                  )}`
                : "—"
            }
          />
          <Detail
            label="Preferred areas"
            value={client.preferredAreas?.join(", ") ?? "—"}
          />
          <Detail label="Timeline" value={client.timeline ?? "—"} />
          <Detail label="Agent" value={client.assignedAgent ?? "—"} />
        </div>

        {showEdit ? (
          <div className="mt-4 border-t border-[var(--portal-panel-border)] pt-4">
            <ClientEditor
              mode="edit"
              client={client}
              agents={agents}
              onSaved={() => setShowEdit(false)}
              onCancel={() => setShowEdit(false)}
            />
          </div>
        ) : null}
      </section>

      <div className="grid gap-3 lg:grid-cols-2">
      <Panel compact heading="Act" className="mt-3">
        <ClientDailyActions clientId={client.id} email={client.email} phone={client.phone} />
      </Panel>


        <Panel compact variant="feature" heading="Next action">
          <div className="font-serif text-lg font-light">
            {client.nextAction?.title ?? "Nothing scheduled"}
          </div>
          {client.nextAction?.occurredAt ? (
            <div className="mt-1 text-xs font-light text-white/55">
              {client.nextAction.occurredAt}
            </div>
          ) : null}
          {client.nextAction?.detail ? (
            <p className="mt-2 text-sm font-light text-white/70">
              {client.nextAction.detail}
            </p>
          ) : null}
        </Panel>

        <Panel compact heading="Last contact">
          <div className="font-serif text-lg font-light text-[var(--portal-navy)]">
            {client.lastContact
              ? channelLabel(client.lastContact.channel)
              : "No contact yet"}
          </div>
          <div className="mt-1 text-xs font-light text-black/45">
            {client.lastContact?.occurredAt ?? "—"}
          </div>
          {client.lastContact?.summary ? (
            <p className="mt-2 line-clamp-3 text-sm font-light leading-6 text-black/55">
              {client.lastContact.summary}
            </p>
          ) : null}
        </Panel>
      </div>

      <Panel
        compact
        heading="Interests"
        action={
          <span className="text-xs font-light text-black/35">
            {client.propertyInterests.length}
          </span>
        }
        divider
        flush
      >
        {client.propertyInterests.length > 0 ? (
          <div className="grid sm:grid-cols-2">
            {client.propertyInterests.map((interest) => (
              <div
                key={interest.id}
                className="flex items-center gap-3 border-b border-[var(--portal-panel-border)] px-4 py-2.5 last:border-b-0 sm:odd:border-r"
              >
                {interest.heroMediaId ? (
                  <img
                    src={`/api/media/${interest.heroMediaId}`}
                    alt={interest.propertyName}
                    className="h-12 w-16 shrink-0 rounded-md object-cover"
                  />
                ) : (
                  <div className="h-12 w-16 shrink-0 rounded-md bg-gradient-to-br from-[var(--portal-blue-pale)] to-[var(--portal-navy-soft)]" />
                )}
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium text-[var(--portal-navy)]">
                    {interest.propertyName}
                  </div>
                  <div className="truncate text-xs font-light text-black/45">
                    {formatCurrency(interest.price)}
                    {interest.location ? ` · ${interest.location}` : ""}
                  </div>
                </div>
                <span className="shrink-0 rounded-full bg-[var(--portal-blue-pale)] px-2 py-0.5 text-[9px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                  {interestStatusLabel(interest.status)}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <p className="px-4 py-6 text-sm font-light text-black/40">
            No property interests yet.
          </p>
        )}
      </Panel>

      <div className="grid gap-3 lg:grid-cols-[minmax(0,1.35fr)_minmax(0,1fr)]">
        <Panel compact heading="Timeline" divider flush>
          {client.interactions.length > 0 ? (
            client.interactions.slice(0, 8).map((interaction) => (
              <div
                key={interaction.id}
                className="grid gap-1 border-b border-[var(--portal-panel-border)] px-4 py-2 last:border-b-0 sm:grid-cols-[9rem_1fr]"
              >
                <div className="text-[11px] font-light text-black/40">
                  {interaction.occurredAt}
                </div>
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium">
                    <span className="mr-2 text-[10px] font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                      {channelLabel(interaction.channel)}
                    </span>
                    {interaction.title}
                  </div>
                  {interaction.summary ? (
                    <p className="mt-0.5 line-clamp-2 text-xs font-light leading-5 text-black/50">
                      {interaction.summary}
                    </p>
                  ) : null}
                </div>
              </div>
            ))
          ) : (
            <p className="px-4 py-6 text-sm font-light text-black/40">
              No interaction history yet.
            </p>
          )}
        </Panel>

        <div className="flex flex-col gap-3">
          <Panel compact variant="soft" heading="Notes">
            <p className="text-sm font-light leading-6 text-[var(--portal-text)]/75">
              {client.notes ?? "No notes yet."}
            </p>
            {client.priorities && client.priorities.length > 0 ? (
              <div className="mt-3 flex flex-wrap gap-1.5">
                {client.priorities.map((priority) => (
                  <span
                    key={priority}
                    className="rounded-full border border-[var(--portal-panel-border)] bg-white/50 px-2.5 py-1 text-[10px] font-light text-[var(--portal-navy-soft)]"
                  >
                    {priority}
                  </span>
                ))}
              </div>
            ) : null}
          </Panel>
          <Panel compact heading="Log a note">
            <InteractionLogForm personId={client.id} />
          </Panel>
        </div>
      </div>
    </div>
  )
}

function Detail({
  label,
  value,
}: {
  label: string
  value: string
}) {
  return (
    <div>
      <div className="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">
        {label}
      </div>
      <div className="mt-1 text-sm font-light leading-5 text-black/70">
        {value}
      </div>
    </div>
  )
}
