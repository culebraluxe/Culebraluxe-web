"use client"

import { useCallback, useEffect, useState } from "react"
import { Panel } from "@/components/portal/panel"
import { PortalInput } from "@/components/portal/ui/portal-field"
import {
  PortalPagination,
  PortalTable,
  PortalTableBody,
  PortalTableCell,
  PortalTableHead,
  PortalTableHeader,
  PortalTableRow,
} from "@/components/portal/ui/portal-table"
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
          <PortalInput
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search imported contacts…"
            aria-label="Search imported contacts"
            className="min-h-9 w-56 bg-white/70"
          />
        </div>
      }
    >

      <div className="hidden md:block">
        <PortalTable>
          <PortalTableHead>
            <PortalTableRow className="hover:bg-transparent">
              <PortalTableHeader>Name</PortalTableHeader>
              <PortalTableHeader>Organization</PortalTableHeader>
              <PortalTableHeader>Email</PortalTableHeader>
              <PortalTableHeader>Phone</PortalTableHeader>
              <PortalTableHeader>Location</PortalTableHeader>
              <PortalTableHeader>Source</PortalTableHeader>
              <PortalTableHeader>Status</PortalTableHeader>
            </PortalTableRow>
          </PortalTableHead>
          <PortalTableBody>
            {rows.length > 0 ? (
              rows.map((row: ImportedContact) => (
                <PortalTableRow key={row.id}>
                  <PortalTableCell>
                    <div className="font-serif text-lg font-light text-[var(--portal-navy)]">
                      {row.displayName}
                    </div>
                    {row.sourceContactId ? (
                      <div className="mt-0.5 truncate text-[10px] font-light uppercase tracking-[0.08em] text-black/30">
                        {row.sourceContactId}
                      </div>
                    ) : null}
                  </PortalTableCell>
                  <PortalTableCell>{row.organization ?? "—"}</PortalTableCell>
                  <PortalTableCell>{row.email ?? "—"}</PortalTableCell>
                  <PortalTableCell>{row.phone ?? "—"}</PortalTableCell>
                  <PortalTableCell>{row.displayAddress ?? "—"}</PortalTableCell>
                  <PortalTableCell>
                    <span className="text-xs font-light text-black/55">
                      {sourceLabel(row.source)}
                    </span>
                  </PortalTableCell>
                  <PortalTableCell>
                    <StatusBadge status={row.reconciliationStatus} />
                  </PortalTableCell>
                </PortalTableRow>
              ))
            ) : (
              <PortalTableRow>
                <PortalTableCell colSpan={7} className="py-12 text-center text-black/45">
                  {loading ? "Loading imported contacts…" : "No imported contacts match."}
                </PortalTableCell>
              </PortalTableRow>
            )}
          </PortalTableBody>
        </PortalTable>
      </div>

      <div className="divide-y divide-[var(--portal-border)] md:hidden">
        {rows.length > 0 ? (
          rows.map((row: ImportedContact) => (
            <article key={row.id} className="space-y-3 px-4 py-4">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <h3 className="font-serif text-lg font-light text-[var(--portal-navy)]">
                    {row.displayName}
                  </h3>
                  {row.sourceContactId ? (
                    <p className="mt-0.5 truncate text-[10px] font-light uppercase tracking-[0.08em] text-black/30">
                      {row.sourceContactId}
                    </p>
                  ) : null}
                </div>
                <StatusBadge status={row.reconciliationStatus} />
              </div>
              <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                <div>
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Organization</dt>
                  <dd className="mt-1 font-light text-black/65">{row.organization ?? "—"}</dd>
                </div>
                <div>
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Source</dt>
                  <dd className="mt-1 font-light text-black/65">{sourceLabel(row.source)}</dd>
                </div>
                <div className="col-span-2">
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Email</dt>
                  <dd className="mt-1 break-words font-light text-black/65">{row.email ?? "—"}</dd>
                </div>
                <div className="col-span-2">
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Phone</dt>
                  <dd className="mt-1 font-light text-black/65">{row.phone ?? "—"}</dd>
                </div>
                <div className="col-span-2">
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Location</dt>
                  <dd className="mt-1 font-light text-black/65">{row.displayAddress ?? "—"}</dd>
                </div>
              </dl>
            </article>
          ))
        ) : (
          <p className="px-4 py-10 text-center text-sm font-light text-black/45">
            {loading ? "Loading imported contacts…" : "No imported contacts match."}
          </p>
        )}
      </div>

      <PortalPagination
        page={page}
        pageCount={totalPages}
        totalLabel={`${total.toLocaleString()} total`}
        onPrevious={() => {
          const next = page - 1
          setPage(next)
          void load(debouncedSearch, next)
        }}
        onNext={() => {
          const next = page + 1
          setPage(next)
          void load(debouncedSearch, next)
        }}
      />
    </Panel>
  )
}
