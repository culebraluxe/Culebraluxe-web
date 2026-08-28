"use client"

import { useCallback, useEffect, useState } from "react"
import type {
  Client,
  ClientStatus,
} from "@/lib/portal/types"
import type { ClientSummary, ClientsPageResult } from "@/db/clients"
import { Panel } from "@/components/portal/panel"
import { ClientEditor } from "@/components/portal/client-editor"
import type { ClientEditorAgent } from "@/components/portal/client-editor"
import { ContactHistory } from "@/components/portal/contact-history"
import { ClientCard } from "@/components/portal/client-card"
import { ClientNotes } from "@/components/portal/client-notes"
import {
  CommandStatus,
  CommandStatusBand,
} from "@/components/portal/command-status-band"
import type { CommandStatusTone } from "@/components/portal/command-status-band"
import {
  formatCurrency,
  formatPhone,
  ghostBtn,
  interestStatusLabel,
  roleLabel,
  statusLabel,
  statusDot,
} from "@/components/portal/client-display"

const LIST_PAGE_SIZE = 50

// Minimal, honest Command slot for the Clients working pane. Client-level AI
// orchestration is a staged capability — this band establishes the reusable
// Command + Status pattern (consistent with Forms) without inventing a large
// AI workflow. Submit reports the intent back through the Status panel.
function ClientCommand({
  clientName,
  onRun,
}: {
  clientName: string
  onRun: (prompt: string) => void
}) {
  const [prompt, setPrompt] = useState("")
  const [note, setNote] = useState<string | null>(null)

  function submit() {
    const text = prompt.trim()
    if (!text) {
      setNote("Tell me what you'd like to do for this client, then tap Go.")
      return
    }
    onRun(text)
    setPrompt("")
    setNote("Command noted — the Clients assistant is a staged capability.")
  }

  return (
    <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] px-4 py-3 sm:px-5">
      <div className="flex flex-wrap items-end gap-2">
        <div className="min-w-0 flex-1">
          <p className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
            Command
          </p>
          <input
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault()
                submit()
              }
            }}
            placeholder={`What should we do for ${clientName}?`}
            className="mt-1.5 block h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 font-serif text-[15px] font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
          />
        </div>
        <button
          type="button"
          onClick={submit}
          className="inline-flex h-10 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)]"
        >
          Go
        </button>
      </div>
      <p className="mt-2 text-center text-[11px] font-light text-black/45">
        {note ??
          "Client-level AI assistance is staged — this band sets up the pattern without inventing a workflow."}
      </p>
    </section>
  )
}

function statusForClient(client: Client) {
  const bits = [roleLabel(client.role), statusLabel(client.status)]
  if (client.assignedAgent) bits.push(`Agent: ${client.assignedAgent}`)
  return bits.join(" · ")
}

function toneForStatus(status: ClientStatus): CommandStatusTone {
  return status === "active" ? "success" : "neutral"
}

export function ClientManager() {
  const [query, setQuery] = useState("")
  const [debouncedQuery, setDebouncedQuery] = useState("")
  const [page, setPage] = useState(1)
  const [list, setList] = useState<ClientSummary[]>([])
  const [total, setTotal] = useState(0)
  const [loadingList, setLoadingList] = useState(true)
  const [selectedClientId, setSelectedClientId] = useState<string | null>(null)
  const [detail, setDetail] = useState<Client | null>(null)
  const [loadingDetail, setLoadingDetail] = useState(false)
  const [agents, setAgents] = useState<ClientEditorAgent[]>([])
  const [showCreate, setShowCreate] = useState(false)
  const [showEdit, setShowEdit] = useState(false)

  // Debounce search so typing doesn't fire a request per keystroke.
  useEffect(() => {
    const id = setTimeout(() => setDebouncedQuery(query), 250)
    return () => clearTimeout(id)
  }, [query])

  // Load assignable agents once (small, bounded) for the New/Edit forms.
  useEffect(() => {
    let active = true
    fetch("/api/portal/clients/agents")
      .then((r) => (r.ok ? r.json() : []))
      .then((rows) => {
        if (active) setAgents(rows as ClientEditorAgent[])
      })
      .catch(() => {})
    return () => {
      active = false
    }
  }, [])

  // Load the bounded people page (server-side search + paging). Reset to page 1
  // whenever the search term changes.
  const loadList = useCallback(async (q: string, p: number) => {
    setLoadingList(true)
    const params = new URLSearchParams({
      view: "directory",
      search: q,
      page: String(p),
      pageSize: String(LIST_PAGE_SIZE),
      sort: "name",
    })
    try {
      const res = await fetch(`/api/portal/clients?${params.toString()}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = (await res.json()) as ClientsPageResult
      setList(json.rows)
      setTotal(json.total)
      setSelectedClientId((prev) => prev ?? json.rows[0]?.id ?? null)
    } catch (err) {
      console.error("Failed to load clients:", err)
      setList([])
      setTotal(0)
    } finally {
      setLoadingList(false)
    }
  }, [])

  useEffect(() => {
    setPage(1)
    void loadList(debouncedQuery, 1)
  }, [debouncedQuery, loadList])

  // Load the selected person's full detail independently — the detail pane
  // never requires loading every Person.
  useEffect(() => {
    if (!selectedClientId) {
      setDetail(null)
      setLoadingDetail(false)
      return
    }
    let active = true
    setLoadingDetail(true)
    fetch(`/api/portal/clients/${selectedClientId}`)
      .then((r) => r.json())
      .then((json) => {
        if (active) setDetail((json as { client?: Client | null })?.client ?? null)
      })
      .catch(() => {
        if (active) setDetail(null)
      })
      .finally(() => {
        if (active) setLoadingDetail(false)
      })
    return () => {
      active = false
    }
  }, [selectedClientId])

  const selectedClient = detail

  // Keyboard navigation over the current bounded page.
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
      const ids = list.map((client) => client.id)
      const index = selectedClientId ? ids.indexOf(selectedClientId) : -1
      if (event.key === "ArrowDown" && index >= 0 && index < ids.length - 1) {
        event.preventDefault()
        setSelectedClientId(ids[index + 1])
        setShowCreate(false)
        setShowEdit(false)
      }
      if (event.key === "ArrowUp" && index > 0) {
        event.preventDefault()
        setSelectedClientId(ids[index - 1])
        setShowCreate(false)
        setShowEdit(false)
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [list, selectedClientId])

  const totalPages = Math.max(1, Math.ceil(total / LIST_PAGE_SIZE))

  function selectClient(id: string) {
    setSelectedClientId(id)
    setShowCreate(false)
    setShowEdit(false)
  }

  return (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="grid min-h-0 flex-1 gap-4 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[200px_minmax(0,1fr)]">
        <aside className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
            <div className="mb-2 flex items-center justify-between gap-2">
              <span className="text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                People · {total.toLocaleString()}
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
            {list.length === 0 ? (
              <p className="px-3 py-6 text-sm font-light text-black/40">
                {loadingList ? "Loading…" : "No matching clients."}
              </p>
            ) : (
              list.map((client) => {
                const selected = selectedClientId === client.id
                return (
                  <button
                    type="button"
                    key={client.id}
                    onClick={() => selectClient(client.id)}
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
                      className={`h-1.5 w-1.5 shrink-0 rounded-full ${statusDot(client.status)}`}
                      aria-label={statusLabel(client.status as ClientStatus)}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-medium text-[var(--portal-navy)]">
                        {client.nameResolved ? client.displayName : "Unknown contact"}
                      </div>
                      <div className="truncate text-[11px] font-light text-black/45">
                        {formatPhone(client.primaryPhone) ??
                          client.primaryEmail ??
                          roleLabel(client.role as Client["role"])}
                      </div>
                      {client.relationshipActivity.observedCommunicationCount > 0 ? (
                        <div className="truncate text-[10px] font-light text-black/35">
                          {client.relationshipActivity.observedCommunicationCount.toLocaleString()} observed
                          {client.relationshipActivity.twoWay ? " · two-way" : ""}
                        </div>
                      ) : null}
                    </div>
                  </button>
                )
              })
            )}
          </div>
          <div className="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-1.5">
            <button
              type="button"
              disabled={page <= 1}
              onClick={() => {
                const next = page - 1
                setPage(next)
                void loadList(debouncedQuery, next)
              }}
              className="inline-flex min-h-7 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-35"
            >
              ← Prev
            </button>
            <span className="text-[10px] font-light uppercase tracking-[0.12em] text-black/40">
              Page {page} / {totalPages}
            </span>
            <button
              type="button"
              disabled={page >= totalPages}
              onClick={() => {
                const next = page + 1
                setPage(next)
                void loadList(debouncedQuery, next)
              }}
              className="inline-flex min-h-7 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-35"
            >
              Next →
            </button>
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
            <Panel compact heading={loadingDetail ? "Loading" : "No clients yet"}>
              <p className="text-sm font-light text-black/45">
                {loadingDetail
                  ? "Loading client…"
                  : "Add a client to start a relationship file."}
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
  const [statusText, setStatusText] = useState(() => statusForClient(client))
  const [statusTone, setStatusTone] = useState<CommandStatusTone>(() =>
    toneForStatus(client.status)
  )

  // Reset the band's status when a different client is selected.
  useEffect(() => {
    setStatusText(statusForClient(client))
    setStatusTone(toneForStatus(client.status))
  }, [client])

  return (
    <div className="flex flex-col gap-3">
      {/* Command + Status band — only in the main workspace (right of the People
          rail); the People rail stays tall so more clients remain visible. */}
      <CommandStatusBand
        ratio="balanced"
        command={
          <ClientCommand
            clientName={client.displayName}
            onRun={(prompt) => {
              setStatusText(`“${prompt}” noted for ${client.displayName}.`)
              setStatusTone("neutral")
            }}
          />
        }
        status={
          <CommandStatus label="Status" tone={statusTone}>
            {statusText}
          </CommandStatus>
        }
      />

      {/* Client Card + Contact History — equal 50/50 siblings, top AND bottom
          aligned (the card sets the row height; history fills it and scrolls
          internally). */}
      <div className="grid gap-3 lg:grid-cols-2 lg:gap-4">
        <ClientCard
          client={client}
          agents={agents}
          showEdit={showEdit}
          setShowEdit={setShowEdit}
        />
        <ContactHistory
          clientId={client.id}
          clientName={client.displayName}
          relationshipActivity={client.relationshipActivity}
          email={client.email}
          phone={client.phone}
        />
      </div>

      {/* Interests + Notes — equal-width lower row. */}
      <div className="grid gap-3 lg:grid-cols-2 lg:gap-4">
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
          className="h-full"
        >
          {client.propertyInterests.length > 0 ? (
            <div className="grid">
              {client.propertyInterests.map((interest) => (
                <div
                  key={interest.id}
                  className="flex items-center gap-3 border-b border-[var(--portal-panel-border)] px-4 py-2.5 last:border-b-0"
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

        <ClientNotes personId={client.id} initialNotes={client.notes} />
      </div>
    </div>
  )
}
