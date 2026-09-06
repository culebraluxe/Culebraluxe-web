"use client"

import { useEffect, useMemo } from "react"
import {
  Calendar,
  Home,
  Mail,
  MapPin,
  MessageSquare,
  Phone,
  Video,
  type LucideIcon,
} from "lucide-react"

import { Panel } from "@/components/portal/panel"
import {
  CommandStatus,
  CommandStatusBand,
  type CommandStatusTone,
} from "@/components/portal/command-status-band"
import {
  formatCurrency,
  formatPhone,
  roleLabel,
  statusDot,
  statusLabel,
} from "@/components/portal/client-display"
import type { ClientRole } from "@/lib/portal/types"
import type { PropertyAddressDto } from "@/services/property"
import {
  ClientLensController,
  HttpClientLensSource,
  type ClientLensChannelModel,
  type ClientLensSource,
} from "@/ui/client-lens"
import { usePageController } from "@/ui/runtime"

type ClientLensModel = Readonly<ReturnType<ClientLensController["snapshot"]>>

function formatDateTime(value: string | null): string {
  if (!value) return "—"
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  })
}

function formatAddress(address: PropertyAddressDto): string {
  return [
    address.addressLine1,
    address.neighborhood,
    address.city,
    [address.stateOrProvince, address.postalCode].filter(Boolean).join(" ") || null,
    address.country,
  ]
    .filter((value): value is string => Boolean(value?.trim()))
    .join(", ") || "Address not available"
}

function channelIcon(slot: ClientLensChannelModel["slot"]): LucideIcon {
  switch (slot) {
    case "phone": return Phone
    case "gmail": return Mail
    case "facetime": return Video
    case "calendar": return Calendar
    default: return MessageSquare
  }
}

function RelationshipChannelRow({ channel }: { channel: ClientLensChannelModel }) {
  const Icon = channelIcon(channel.slot)
  return (
    <li className="border-b border-white/10 px-4 py-3 last:border-b-0">
      <div className="flex items-start gap-3">
        <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-white/15 bg-white/5">
          <Icon className="h-4 w-4 text-white/70" aria-hidden />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-3">
            <div className="text-xs font-medium text-white">{channel.label}</div>
            <div className="text-[10px] font-light uppercase tracking-[0.12em] text-white/45">
              {channel.connected ? `${channel.totalCount.toLocaleString()} observed` : "Not connected"}
            </div>
          </div>
          {channel.connected ? (
            <>
              <div className="mt-1 text-[10px] font-light text-white/50">
                {channel.inboundCount.toLocaleString()} in · {channel.outboundCount.toLocaleString()} out
                {channel.twoWay ? " · two-way" : ""}
              </div>
              <div className="mt-1 text-[11px] font-light text-white/65">
                Last: {formatDateTime(channel.lastContactAt)}
              </div>
              <div className="mt-1 truncate text-xs font-light text-white/85">
                {channel.lastContext ?? "No recent context captured."}
              </div>
            </>
          ) : (
            <div className="mt-1 text-xs font-light text-white/40">No source evidence yet.</div>
          )}
        </div>
      </div>
    </li>
  )
}

function DetailField({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">{label}</div>
      <div className="mt-1 break-words text-sm font-light leading-5 text-black/70">{value}</div>
    </div>
  )
}

function ClientGrokPlaceholder({ clientName }: { clientName: string | null }) {
  return (
    <section className="portal-glass-panel portal-glass-panel-lifted flex h-full min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
      <div className="shrink-0 border-b border-[var(--portal-panel-border)] px-4 py-2.5">
        <p className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
          Grok
        </p>
      </div>
      <div className="flex min-h-0 flex-1 items-center gap-2 px-4 py-2.5">
        <input
          type="text"
          disabled
          placeholder={clientName ? `Ask Grok about ${clientName}…` : "Ask Grok about this client…"}
          className="h-10 min-w-0 flex-1 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55 px-3 font-serif text-[15px] font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35 disabled:cursor-not-allowed disabled:opacity-70"
        />
        <button
          type="button"
          disabled
          className="inline-flex h-10 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:cursor-not-allowed disabled:opacity-45"
        >
          Go
        </button>
      </div>
    </section>
  )
}

function clientLensStatus(
  model: ClientLensModel,
  notesDirty: boolean,
): { tone: CommandStatusTone; text: string } {
  const errors = [
    model.listError ? `Client list: ${model.listError}` : null,
    model.clientError ? `Client: ${model.clientError}` : null,
    model.channelsError ? `Relationship: ${model.channelsError}` : null,
    model.propertyError ? `Property: ${model.propertyError}` : null,
  ].filter((value): value is string => Boolean(value))

  if (errors.length > 0) {
    return { tone: "danger", text: errors.join(" · ") }
  }

  if (model.notesSaving) {
    return { tone: "neutral", text: "Saving client notes…" }
  }

  if (model.notesStatus) {
    return model.notesStatus === "Saved"
      ? { tone: "success", text: "Client notes saved." }
      : { tone: "danger", text: `Notes: ${model.notesStatus}` }
  }

  if (notesDirty) {
    return { tone: "warning", text: "Unsaved note changes." }
  }

  const loading = [
    model.listLoading ? "client list" : null,
    model.clientLoading ? "client" : null,
    model.channelsLoading ? "relationship" : null,
    model.propertyLoading ? "property context" : null,
  ].filter((value): value is string => Boolean(value))

  if (loading.length > 0) {
    return { tone: "neutral", text: `Loading ${loading.join(", ")}…` }
  }

  if (model.client) {
    return {
      tone: "success",
      text: `Ready · ${model.client.displayName}`,
    }
  }

  return { tone: "neutral", text: "Ready." }
}

function PropertyContextPanel({
  model,
}: {
  model: ClientLensModel
}) {
  const context = model.propertyContext
  const canonicalCount = context?.properties.length ?? 0
  const observedCount = context?.observedAddresses.length ?? 0

  return (
    <Panel
      compact
      heading="Property Context"
      action={
        <span className="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
          {model.propertyLoading
            ? "Loading…"
            : `${canonicalCount} properties · ${observedCount} address observations`}
        </span>
      }
    >
      {model.propertyLoading ? (
        <p className="text-sm font-light text-black/45">Loading Property service…</p>
      ) : model.propertyError ? (
        <p className="text-sm font-light text-[var(--portal-archive)]">
          Property context unavailable · {model.propertyError}
        </p>
      ) : !context || (canonicalCount === 0 && observedCount === 0) ? (
        <p className="text-sm font-light text-black/45">
          No Property relationship or structured address evidence yet.
        </p>
      ) : (
        <div className="space-y-4">
          {canonicalCount > 0 ? (
            <section>
              <div className="mb-2 text-[10px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">
                Canonical Property
              </div>
              <div className="space-y-2">
                {context.properties.map(({ property, relationStatus }) => (
                  <div
                    key={property.id}
                    className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-3 py-2.5"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <Home className="h-3.5 w-3.5 shrink-0 text-[var(--portal-navy-soft)]" aria-hidden />
                          <div className="truncate font-serif text-base font-light text-[var(--portal-navy)]">
                            {property.displayName}
                          </div>
                        </div>
                        <div className="mt-1 pl-5 text-xs font-light leading-5 text-black/55">
                          {formatAddress(property.address)}
                        </div>
                      </div>
                      <span className="shrink-0 rounded-full bg-[var(--portal-blue-pale)] px-2 py-1 text-[9px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                        {relationStatus ?? "linked"}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            </section>
          ) : null}

          {observedCount > 0 ? (
            <section>
              <div className="mb-2 flex items-center justify-between gap-3">
                <div className="text-[10px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">
                  Structured Address Evidence
                </div>
                <span className="text-[9px] font-light uppercase tracking-[0.1em] text-black/35">
                  Apple Contacts
                </span>
              </div>
              <div className="space-y-2">
                {context.observedAddresses.map((observation) => (
                  <div
                    key={observation.sourceKey}
                    className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 px-3 py-2.5"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2 text-xs font-medium text-[var(--portal-navy)]">
                          <MapPin className="h-3.5 w-3.5 shrink-0 text-[var(--portal-gold-muted)]" aria-hidden />
                          {observation.sourceLabel || "Contact address"}
                        </div>
                        <div className="mt-1 pl-5 text-xs font-light leading-5 text-black/60">
                          {formatAddress(observation.address)}
                        </div>
                      </div>
                      <span className="shrink-0 text-[9px] font-light uppercase tracking-[0.1em] text-black/40">
                        {observation.matchedPropertyId ? "Matched" : "Candidate"}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            </section>
          ) : null}
        </div>
      )}
    </Panel>
  )
}

export function ClientLens({ source }: { source?: ClientLensSource } = {}) {
  const controller = useMemo(
    () => new ClientLensController(source ?? new HttpClientLensSource()),
    [source],
  )
  const model = usePageController(controller)

  useEffect(() => {
    void controller.dispatch({ operation: "clientLens.load", payload: {} })
    // Keep the memoized controller alive through React StrictMode cleanup/setup.
  }, [controller])

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const target = event.target
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement
      ) return

      const ids = model.list.map((client) => client.id)
      const index = model.selectedClientId ? ids.indexOf(model.selectedClientId) : -1
      if (event.key === "ArrowDown" && index >= 0 && index < ids.length - 1) {
        event.preventDefault()
        void controller.dispatch({
          operation: "clientLens.selectClient",
          payload: { personId: ids[index + 1] },
        })
      }
      if (event.key === "ArrowUp" && index > 0) {
        event.preventDefault()
        void controller.dispatch({
          operation: "clientLens.selectClient",
          payload: { personId: ids[index - 1] },
        })
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [controller, model.list, model.selectedClientId])

  const client = model.client
  const notesDirty = model.notesDraft !== model.notesSaved
  const observed = client?.relationshipActivity?.observedCommunicationCount ?? 0
  const connectedSources = model.channels.filter((channel) => channel.connected).length
  const status = clientLensStatus(model, notesDirty)

  return (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="flex items-center justify-between gap-4 px-1">
        <div>
          <div className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
            Architecture Sidecar
          </div>
          <h1 className="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">Client Lens</h1>
        </div>
        <div className="text-right text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
          MVI/MVU · real CRM data
        </div>
      </div>

      <div className="grid min-h-0 flex-1 gap-4 lg:h-[calc(100dvh-10.5rem)] lg:grid-cols-[220px_minmax(0,1fr)]">
        <aside className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="shrink-0 border-b border-[var(--portal-panel-border)] p-3">
            <div className="mb-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
              People · {model.total.toLocaleString()}
            </div>
            <input
              type="search"
              value={model.query}
              onChange={(event) => {
                void controller.dispatch({
                  operation: "clientLens.queryChanged",
                  payload: { query: event.target.value },
                })
              }}
              placeholder="Search…"
              className="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-2.5 py-2 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
            />
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto">
            {model.list.length === 0 ? (
              <p className="px-3 py-6 text-sm font-light text-black/40">
                {model.listLoading ? "Loading…" : model.listError ?? "No matching clients."}
              </p>
            ) : (
              model.list.map((item) => {
                const selected = model.selectedClientId === item.id
                return (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => {
                      void controller.dispatch({
                        operation: "clientLens.selectClient",
                        payload: { personId: item.id },
                      })
                    }}
                    className={[
                      "flex w-full items-start gap-2 border-b border-[var(--portal-panel-border)] px-3 py-2.5 text-left transition",
                      selected
                        ? "border-l-2 border-l-[var(--portal-gold)] bg-white/45"
                        : "border-l-2 border-l-transparent hover:bg-white/25",
                    ].join(" ")}
                  >
                    <span className={`mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full ${statusDot(item.status)}`} />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-medium text-[var(--portal-navy)]">
                        {item.nameResolved ? item.displayName : "Unknown contact"}
                      </div>
                      <div className="truncate text-[11px] font-light text-black/45">
                        {formatPhone(item.primaryPhone) ?? item.primaryEmail ?? roleLabel(item.role as ClientRole)}
                      </div>
                      <div className="mt-0.5 truncate text-[10px] font-light text-black/35">
                        {item.relationshipActivity.observedCommunicationCount.toLocaleString()} observed
                        {item.relationshipActivity.twoWay ? " · two-way" : ""}
                      </div>
                    </div>
                  </button>
                )
              })
            )}
          </div>

          <div className="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-2">
            <button
              type="button"
              disabled={model.page <= 1}
              onClick={() => void controller.dispatch({ operation: "clientLens.previousPage", payload: {} })}
              className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30"
            >
              ← Prev
            </button>
            <span className="text-[10px] font-light text-black/40">{model.page} / {model.pageCount}</span>
            <button
              type="button"
              disabled={model.page >= model.pageCount}
              onClick={() => void controller.dispatch({ operation: "clientLens.nextPage", payload: {} })}
              className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30"
            >
              Next →
            </button>
          </div>
        </aside>

        <div className="flex min-h-0 flex-col gap-3">
          <CommandStatusBand
            ratio="balanced"
            command={<ClientGrokPlaceholder clientName={client?.displayName ?? null} />}
            status={
              <CommandStatus label="Status" tone={status.tone}>
                {status.text}
              </CommandStatus>
            }
          />

          <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(300px,0.8fr)]">
            <main className="min-h-0 overflow-y-auto">
              {!client ? (
                <Panel compact heading={model.clientLoading ? "Loading client" : "Client"}>
                  <p className="text-sm font-light text-black/45">
                    {model.clientLoading ? "Loading selected client…" : model.clientError ?? "Select a client."}
                  </p>
                </Panel>
              ) : (
                <div className="flex flex-col gap-4">
                  <Panel compact heading="Client">
                    <div className="flex items-start justify-between gap-3 border-b border-[var(--portal-panel-border)] pb-4">
                      <div>
                        <h2 className="font-serif text-2xl font-light text-[var(--portal-navy)]">{client.displayName}</h2>
                        <div className="mt-1 flex items-center gap-2 text-xs font-light text-black/50">
                          <span>{roleLabel(client.role)}</span>
                          <span>·</span>
                          <span>{statusLabel(client.status)}</span>
                        </div>
                      </div>
                      <div className="text-right text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
                        {observed.toLocaleString()} observed<br />
                        {connectedSources} / 6 sources
                      </div>
                    </div>

                    <div className="mt-4 grid gap-4 sm:grid-cols-2">
                      <DetailField label="Phone" value={formatPhone(client.phone) ?? "—"} />
                      <DetailField label="Email" value={client.email ?? "—"} />
                      <DetailField label="Agent" value={client.assignedAgent ?? "—"} />
                      <DetailField label="Timeline" value={client.timeline ?? "—"} />
                      <DetailField
                        label="Budget"
                        value={
                          client.budgetMin || client.budgetMax
                            ? `${formatCurrency(client.budgetMin)} – ${formatCurrency(client.budgetMax)}`
                            : "—"
                        }
                      />
                    </div>
                  </Panel>

                  <PropertyContextPanel model={model} />

                  <Panel compact heading="Notes" className="flex min-h-[14rem] flex-col">
                    <textarea
                      value={model.notesDraft}
                      onChange={(event) => {
                        void controller.dispatch({
                          operation: "clientLens.notesChanged",
                          payload: { notes: event.target.value },
                        })
                      }}
                      placeholder="What do I want to remember about this client?"
                      className="min-h-[9rem] w-full flex-1 resize-none rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 p-3 font-serif text-[15px] font-light leading-6 text-[var(--portal-navy)] outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
                    />
                    <div className="mt-3 flex items-center justify-between gap-3">
                      <span className="text-[10px] font-light text-black/45">{model.notesStatus ?? ""}</span>
                      <button
                        type="button"
                        disabled={!notesDirty || model.notesSaving}
                        onClick={() => void controller.dispatch({ operation: "clientLens.saveNotes", payload: {} })}
                        className="inline-flex min-h-9 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:opacity-35"
                      >
                        {model.notesSaving ? "Saving…" : "Save"}
                      </button>
                    </div>
                  </Panel>
                </div>
              )}
            </main>

            <aside className="min-h-0 overflow-hidden rounded-[var(--portal-panel-radius)] bg-[var(--portal-navy-deep)] text-white shadow-sm">
              <div className="border-b border-white/10 px-4 py-3">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <div className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">Relationship</div>
                    <h2 className="mt-1 font-serif text-xl font-light">Six-channel lens</h2>
                  </div>
                  <div className="text-right text-[10px] font-light text-white/45">
                    {model.channelsLoading ? "Loading…" : `${connectedSources} connected`}
                  </div>
                </div>
              </div>
              <ol className="min-h-0 overflow-y-auto">
                {model.channels.length > 0 ? (
                  model.channels.map((channel) => <RelationshipChannelRow key={channel.slot} channel={channel} />)
                ) : (
                  <li className="px-4 py-6 text-sm font-light text-white/45">
                    {model.channelsLoading
                      ? "Loading relationship sources…"
                      : model.channelsError ?? "Select a client."}
                  </li>
                )}
              </ol>
            </aside>
          </div>
        </div>
      </div>
    </div>
  )
}
