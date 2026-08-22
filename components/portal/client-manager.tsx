"use client"

import Link from "next/link"
import { useMemo, useState } from "react"
import type {
  Client,
  ClientStatus,
  InteractionChannel,
  PropertyInterestStatus,
} from "@/lib/portal/types"
import { Panel } from "@/components/portal/panel"
import { ClientEditor } from "@/components/portal/client-editor"
import type { ClientEditorAgent } from "@/components/portal/client-editor"
import { PageHeader } from "@/components/portal/page-header"

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

function statusClasses(status: ClientStatus) {
  switch (status) {
    case "active":
      return "bg-[var(--portal-success-pale)] text-[var(--portal-success)]"
    case "warm":
      return "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]"
    case "referral":
      return "bg-[var(--portal-neutral-pale)] text-[var(--portal-neutral)]"
    default:
      return "bg-black/5 text-black/50"
  }
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
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
        .includes(normalized)
    )
  }, [clients, query])

  const selectedClient =
    clients.find((client) => client.id === selectedClientId) ??
    clients[0]

  if (!selectedClient) {
    return (
      <div>
        <PageHeader
          eyebrow="Relationships"
          title="Client Manager"
          subtitle="Relationship intelligence across buyers, sellers, and island introductions."
        >
          <button
            type="button"
            onClick={() => setShowCreate(true)}
            className="mt-6 inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-5 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)]"
          >
            New client
          </button>
        </PageHeader>

        {showCreate && (
          <div className="mb-6">
            <ClientEditor
              mode="create"
              agents={agents}
              onSaved={(personId) => {
                setSelectedClientId(personId)
                setShowCreate(false)
              }}
              onCancel={() => setShowCreate(false)}
            />
          </div>
        )}

        <div className="rounded-sm border border-black/10 bg-white p-10">
          <p className="text-sm font-light text-black/45">
            No clients found.
          </p>
        </div>
      </div>
    )
  }

  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Relationships
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Client Manager
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Relationship intelligence across buyers, sellers, and island introductions.
        </p>

        <button
          type="button"
          onClick={() => setShowCreate(true)}
          className="mt-6 inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-5 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)]"
        >
          New client
        </button>
      </div>

      {showCreate && (
        <div className="mb-6">
          <ClientEditor
            mode="create"
            agents={agents}
            onSaved={(personId) => {
              setSelectedClientId(personId)
              setShowCreate(false)
            }}
            onCancel={() => setShowCreate(false)}
          />
        </div>
      )}

      <div className="grid gap-6 xl:grid-cols-[330px_minmax(0,1fr)]">
        <aside className="overflow-hidden rounded-sm border border-black/10 bg-white">
          <div className="border-b border-black/10 p-5">
            <input
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search clients..."
              className="w-full rounded-sm border border-black/10 bg-[var(--portal-surface)] px-4 py-3 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
            />

            <div className="mt-4 flex items-center justify-between">
              <span className="text-xs font-light uppercase tracking-[0.18em] text-black/40">
                Clients
              </span>

              <span className="text-xs font-light text-black/35">
                {filteredClients.length}
              </span>
            </div>
          </div>

          <div>
            {filteredClients.map((client) => {
              const selected = client.id === selectedClient.id

              return (
                <button
                  type="button"
                  key={client.id}
                  onClick={() => setSelectedClientId(client.id)}
                  className={[
                    "w-full border-b border-black/5 px-5 py-5 text-left transition",
                    selected
                      ? "bg-[var(--portal-blue-pale)]"
                      : "bg-white hover:bg-[var(--portal-surface)]",
                  ].join(" ")}
                >
                  <div className="flex items-start gap-3">
                    <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] font-serif text-sm font-light text-[var(--portal-navy-soft)]">
                      {client.displayName
                        .split(" ")
                        .slice(0, 2)
                        .map((word) => word[0])
                        .join("")}
                    </div>

                    <div className="min-w-0 flex-1">
                      <div className="flex items-start justify-between gap-3">
                        <div className="truncate text-sm font-medium">
                          {client.displayName}
                        </div>

                        <span
                          className={[
                            "rounded-full px-2 py-1 text-[10px] font-light uppercase tracking-[0.08em]",
                            statusClasses(client.status),
                          ].join(" ")}
                        >
                          {statusLabel(client.status)}
                        </span>
                      </div>

                      <div className="mt-1 text-xs font-light text-black/45">
                        {roleLabel(client.role)}
                        {client.location && ` · ${client.location}`}
                      </div>
                    </div>
                  </div>
                </button>
              )
            })}
          </div>
        </aside>

        <div className="space-y-6">
          <section className="rounded-sm border border-black/10 bg-white p-6 lg:p-8">
            <div className="flex flex-col justify-between gap-6 lg:flex-row lg:items-start">
              <div className="flex items-center gap-3">
                <div className="flex h-14 w-14 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] font-serif text-lg font-light text-[var(--portal-navy-soft)]">
                  {selectedClient.displayName
                    .split(" ")
                    .slice(0, 2)
                    .map((word) => word[0])
                    .join("")}
                </div>

                <div>
                  <h2 className="font-serif text-3xl font-light">
                    {selectedClient.displayName}
                  </h2>

                  <div className="mt-1 flex flex-wrap items-center gap-2 text-sm font-light text-black/45">
                    <span>{roleLabel(selectedClient.role)}</span>

                    {selectedClient.location && (
                      <>
                        <span>·</span>
                        <span>{selectedClient.location}</span>
                      </>
                    )}
                  </div>
                </div>
              </div>

              <div className="flex flex-wrap items-center gap-3">
                <Link
                  href={`/portal/clients/${selectedClient.id}`}
                  className="self-start rounded-full border border-[var(--portal-border)] px-5 py-2.5 text-xs font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]"
                >
                  View dossier
                </Link>

                <button
                  type="button"
                  onClick={() => setShowEdit(!showEdit)}
                  className="self-start rounded-full border border-[var(--portal-border)] px-5 py-2.5 text-xs font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]"
                >
                  {showEdit ? "Close editor" : "Edit profile"}
                </button>
              </div>
            </div>

            <div className="mt-8 grid gap-6 border-t border-black/10 pt-7 md:grid-cols-2 xl:grid-cols-4">
              <Detail
                label="Budget"
                value={
                  selectedClient.budgetMin || selectedClient.budgetMax
                    ? `${formatCurrency(selectedClient.budgetMin)} – ${formatCurrency(
                        selectedClient.budgetMax
                      )}`
                    : "—"
                }
              />

              <Detail
                label="Preferred Areas"
                value={selectedClient.preferredAreas?.join(", ") ?? "—"}
              />

              <Detail
                label="Timeline"
                value={selectedClient.timeline ?? "—"}
              />

              <Detail
                label="Assigned Agent"
                value={selectedClient.assignedAgent ?? "—"}
              />
            </div>

            {showEdit && (
              <div className="mt-8">
                <ClientEditor
                  mode="edit"
                  client={selectedClient}
                  agents={agents}
                  onSaved={() => setShowEdit(false)}
                  onCancel={() => setShowEdit(false)}
                />
              </div>
            )}
          </section>

          <div className="grid gap-6 2xl:grid-cols-[minmax(0,1.45fr)_minmax(300px,.75fr)]">
            <section className="rounded-sm border border-black/10 bg-white">
              <div className="flex items-center justify-between border-b border-black/10 px-6 py-5">
                <div>
                  <h3 className="font-serif text-xl font-light">
                    Active Interests
                  </h3>

                  <p className="mt-1 text-xs font-light text-black/40">
                    Properties currently connected to this client.
                  </p>
                </div>

                <span className="text-xs font-light text-black/35">
                  {selectedClient.propertyInterests.length}
                </span>
              </div>

              {selectedClient.propertyInterests.length > 0 ? (
                <div>
                  {selectedClient.propertyInterests.map((interest) => (
                    <div
                      key={interest.id}
                      className="flex flex-col gap-4 border-b border-black/5 px-6 py-5 last:border-b-0 md:flex-row md:items-center"
                    >
                      {interest.heroMediaId ? (
                        <img
                          src={`/api/media/${interest.heroMediaId}`}
                          alt={interest.propertyName}
                          className="h-20 w-full shrink-0 rounded-sm object-cover md:w-28"
                        />
                      ) : (
                        <div className="h-20 w-full shrink-0 rounded-sm bg-gradient-to-br from-[var(--portal-blue-pale)] via-[var(--portal-mist-3)] to-[var(--portal-blue-gray)] md:w-28" />
                      )}

                      <div className="min-w-0 flex-1">
                        <div className="font-serif text-lg font-light">
                          {interest.propertyName}
                        </div>

                        <div className="mt-1 text-xs font-light text-black/45">
                          {interest.location}
                          {interest.bedrooms &&
                            ` · ${interest.bedrooms} bedrooms`}
                          {interest.descriptor &&
                            ` · ${interest.descriptor}`}
                        </div>

                        <div className="mt-2 text-sm font-light">
                          {formatCurrency(interest.price)}
                        </div>
                      </div>

                      <span className="self-start rounded-full bg-[var(--portal-blue-pale)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)] md:self-center">
                        {interestStatusLabel(interest.status)}
                      </span>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="px-6 py-10 text-sm font-light text-black/40">
                  No active property interests.
                </div>
              )}
            </section>

            <div className="space-y-6">
              <section className="rounded-sm border border-black/10 bg-white p-6">
                <p className="text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Last Contact
                </p>

                <div className="mt-4 font-serif text-xl font-light">
                  {selectedClient.lastContact
                    ? channelLabel(selectedClient.lastContact.channel)
                    : "—"}
                </div>

                <div className="mt-1 text-sm font-light text-black/45">
                  {selectedClient.lastContact?.occurredAt ?? "—"}
                </div>

                {selectedClient.lastContact?.summary && (
                  <p className="mt-4 text-sm font-light leading-6 text-black/55">
                    {selectedClient.lastContact.summary}
                  </p>
                )}
              </section>

              <Panel variant="feature" eyebrow="Next Action">

                <div className="mt-4 font-serif text-xl font-light">
                  {selectedClient.nextAction?.title ?? "Nothing scheduled"}
                </div>

                <div className="mt-1 text-sm font-light text-white/55">
                  {selectedClient.nextAction?.occurredAt ?? ""}
                </div>

                {selectedClient.nextAction?.detail && (
                  <p className="mt-4 text-sm font-light text-white/70">
                    {selectedClient.nextAction.detail}
                  </p>
                )}
              </Panel>
            </div>
          </div>

          <div className="grid gap-6 2xl:grid-cols-[minmax(0,1.3fr)_minmax(300px,.7fr)]">
            <Panel
              variant="standard"
              heading="Interaction Timeline"
              subtitle="Calls, messages, email, meetings, showings and notes."
              divider
              flush
            >

              <div>
                {selectedClient.interactions.length > 0 ? (
                  selectedClient.interactions.map((interaction) => (
                    <div
                      key={interaction.id}
                      className="grid gap-3 border-b border-black/5 px-6 py-5 last:border-b-0 md:grid-cols-[150px_120px_1fr]"
                    >
                      <div className="text-xs font-light text-black/40">
                        {interaction.occurredAt}
                      </div>

                      <div className="text-xs font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                        {channelLabel(interaction.channel)}
                      </div>

                      <div>
                        <div className="text-sm font-medium">
                          {interaction.title}
                        </div>

                        {interaction.summary && (
                          <p className="mt-1 text-sm font-light leading-6 text-black/50">
                            {interaction.summary}
                          </p>
                        )}
                      </div>
                    </div>
                  ))
                ) : (
                  <div className="px-6 py-10 text-sm font-light text-black/40">
                    No interaction history yet.
                  </div>
                )}
              </div>
            </Panel>

            <Panel variant="soft" eyebrow="Relationship Notes">

              <p className="mt-5 text-sm font-light leading-7 text-[var(--portal-text)]/70">
                {selectedClient.notes ?? "No notes yet."}
              </p>

              {selectedClient.priorities &&
                selectedClient.priorities.length > 0 && (
                  <div className="mt-6 flex flex-wrap gap-2">
                    {selectedClient.priorities.map((priority) => (
                      <span
                        key={priority}
                        className="rounded-full border border-[var(--portal-border)] bg-white/70 px-3 py-1.5 text-xs font-light text-[var(--portal-navy-soft)]"
                      >
                        {priority}
                      </span>
                    ))}
                  </div>
                )}
            </Panel>
          </div>
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
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-black/35">
        {label}
      </div>

      <div className="mt-2 text-sm font-light leading-6 text-black/70">
        {value}
      </div>
    </div>
  )
}
