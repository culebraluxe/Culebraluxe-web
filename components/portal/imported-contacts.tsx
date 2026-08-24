"use client"

import { useCallback, useEffect, useState } from "react"
import { Panel } from "@/components/portal/panel"
import type { ImportedContact, ImportedContactsResult } from "@/db/imported-contacts"

// ---------------------------------------------------------------------------
// SUPPORT-2 — Imported Contacts (Apple Contacts) view within the Clients page.
//
// A clearly-labelled, non-canonical read of the relational load projection
// (l_person). Server-side search + pagination via /api/portal/imported-contacts
// so the 2,573-row payload is never sent to the browser in one go. These rows
// are NOT canonical CRM Clients (no promote/merge/reject in this story).
// ---------------------------------------------------------------------------

const PAGE_SIZE = 25

function sourceLabel(source: string) {
  if (source === "apple_contacts") return "Apple Contacts"
  return source
}

function StatusBadge({ status }: { status: string }) {
  const tone =
    status === "unreviewed"
      ? "border-[var(--portal-blue-gray)] text-[var(--portal-blue-gray)]"
      : "border-[var(--portal-gold)] text-[var(--portal-navy-soft)]"
  return (
    <span
      className={`inline-block rounded-full border px-2 py-0.5 text-[9px] font-light uppercase tracking-[0.1em] ${tone}`}
    >
      {status}
    </span>
  )
}

export function ImportedContactsPanel({
  initialTotal,
}: {
  initialTotal: number
}) {
  const [search, setSearch] = useState("")
  const [debouncedSearch, setDebouncedSearch] = useState("")
  const [page, setPage] = useState(1)
  const [data, setData] = useState<ImportedContactsResult | null>(null)
  const [loading, setLoading] = useState(true)

  // Debounce the search input so typing doesn't fire a request per keystroke.
  useEffect(() => {
    const id = setTimeout(() => setDebouncedSearch(search), 250)
    return () => clearTimeout(id)
  }, [search])

  const load = useCallback(async (q: string, p: number) => {
    setLoading(true)
    const url = `/api/portal/imported-contacts?search=${encodeURIComponent(q)}&page=${p}&pageSize=${PAGE_SIZE}`
    try {
      const res = await fetch(url)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = (await res.json()) as ImportedContactsResult
      setData(json)
    } catch (err) {
      console.error("Failed to load imported contacts:", err)
      setData(null)
    } finally {
      setLoading(false)
    }
  }, [])

  // Reset to page 1 whenever the (debounced) search changes.
  useEffect(() => {
    setPage(1)
    void load(debouncedSearch, 1)
  }, [debouncedSearch, load])

  const total = data?.total ?? initialTotal
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))
  const rows = data?.rows ?? []

  return (
    <Panel
      variant="soft"
      eyebrow="Apple Contacts"
      heading="Imported Contacts"
      subtitle={`${total.toLocaleString()} unreviewed contacts staged from Apple Contacts — relational load projection, not canonical CRM clients.`}
      flush
      divider
      action={
        <div className="flex min-h-9 items-center gap-2">
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search imported contacts…"
            className="min-h-9 w-56 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-sm font-light text-[var(--portal-navy)] outline-none transition placeholder:text-black/35 focus:border-[var(--portal-navy)]"
          />
        </div>
      }
    >

      <div className="overflow-x-auto">
        <table className="w-full min-w-[980px] border-collapse">
          <thead>
            <tr className="border-b border-[var(--portal-panel-border)] bg-[var(--portal-blue-pale)]/60">
              <Th>Name</Th>
              <Th>Organization</Th>
              <Th>Email</Th>
              <Th>Phone</Th>
              <Th>Location</Th>
              <Th>Source</Th>
              <Th>Status</Th>
            </tr>
          </thead>
          <tbody>
            {rows.length > 0 ? (
              rows.map((row: ImportedContact) => (
                <tr
                  key={row.id}
                  className="border-b border-[var(--portal-panel-border)] last:border-b-0 hover:bg-[var(--portal-blue-pale)]/40"
                >
                  <Td>
                    <div className="font-serif text-lg font-light text-[var(--portal-navy)]">
                      {row.displayName}
                    </div>
                    {row.sourceContactId ? (
                      <div className="mt-0.5 truncate text-[10px] font-light uppercase tracking-[0.08em] text-black/30">
                        {row.sourceContactId}
                      </div>
                    ) : null}
                  </Td>
                  <Td>{row.organization ?? "—"}</Td>
                  <Td>{row.email ?? "—"}</Td>
                  <Td>{row.phone ?? "—"}</Td>
                  <Td>{row.displayAddress ?? "—"}</Td>
                  <Td>
                    <span className="text-xs font-light text-black/55">
                      {sourceLabel(row.source)}
                    </span>
                  </Td>
                  <Td>
                    <StatusBadge status={row.reconciliationStatus} />
                  </Td>
                </tr>
              ))
            ) : (
              <tr>
                <td colSpan={7} className="px-4 py-12 text-center text-sm font-light text-black/40">
                  {loading ? "Loading imported contacts…" : "No imported contacts match."}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="flex items-center justify-between border-t border-[var(--portal-panel-border)] px-4 py-3">
        <span className="text-xs font-light text-black/45">
          Page {page} of {totalPages} · {total.toLocaleString()} total
        </span>
        <div className="flex gap-2">
          <button
            type="button"
            disabled={page <= 1}
            onClick={() => {
              const next = page - 1
              setPage(next)
              void load(debouncedSearch, next)
            }}
            className="inline-flex min-h-9 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40"
          >
            Previous
          </button>
          <button
            type="button"
            disabled={page >= totalPages}
            onClick={() => {
              const next = page + 1
              setPage(next)
              void load(debouncedSearch, next)
            }}
            className="inline-flex min-h-9 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40"
          >
            Next
          </button>
        </div>
      </div>
    </Panel>
  )
}

function Th({ children }: { children: React.ReactNode }) {
  return (
    <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
      {children}
    </th>
  )
}

function Td({ children }: { children: React.ReactNode }) {
  return <td className="px-4 py-4 align-top text-sm font-light text-black/70">{children}</td>
}
