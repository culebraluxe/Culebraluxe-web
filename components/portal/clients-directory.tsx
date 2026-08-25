"use client"

import { useCallback, useEffect, useState } from "react"
import Link from "next/link"

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
import type { ClientsPageResult } from "@/db/clients"

// ---------------------------------------------------------------------------
// CLIENTS — primary directory, server-side paginated over the canonical
// `person` parent table.
//
// This is a client component that loads its own pages from /api/portal/clients
// (50/page) so the canonical pane renders immediately and never materializes
// the whole person table. Search / filters / sort are applied in SQL; each row
// links to the full dossier (/portal/clients/[personId]). It must not block on
// the Imported Contacts tab, and the Imported tab loads independently.
// ---------------------------------------------------------------------------

const PAGE_SIZE = 50

function roleLabel(role: string) {
  if (role === "both") return "Buyer & Seller"
  return role.charAt(0).toUpperCase() + role.slice(1)
}

function statusLabel(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

export function ClientsDirectory() {
  const [search, setSearch] = useState("")
  const [debouncedSearch, setDebouncedSearch] = useState("")
  const [status, setStatus] = useState("")
  const [role, setRole] = useState("")
  const [sort, setSort] = useState("name")
  const [page, setPage] = useState(1)
  const [data, setData] = useState<ClientsPageResult | null>(null)
  const [loading, setLoading] = useState(true)

  // Debounce search so typing doesn't fire a request per keystroke.
  useEffect(() => {
    const id = setTimeout(() => setDebouncedSearch(search), 250)
    return () => clearTimeout(id)
  }, [search])

  const load = useCallback(async (q: string, p: number, st: string, rl: string, so: string) => {
    setLoading(true)
    const params = new URLSearchParams({
      view: "directory",
      search: q,
      page: String(p),
      pageSize: String(PAGE_SIZE),
      status: st,
      role: rl,
      sort: so,
    })
    try {
      const res = await fetch(`/api/portal/clients?${params.toString()}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = (await res.json()) as ClientsPageResult
      setData(json)
    } catch (err) {
      console.error("Failed to load clients:", err)
      setData(null)
    } finally {
      setLoading(false)
    }
  }, [])

  // Reset to page 1 whenever search / filters / sort change.
  useEffect(() => {
    setPage(1)
    void load(debouncedSearch, 1, status, role, sort)
  }, [debouncedSearch, status, role, sort, load])

  const total = data?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))
  const rows = data?.rows ?? []

  const filterClass =
    "min-h-9 rounded-[var(--portal-tab-radius)] border border-[var(--portal-border)] bg-white/70 px-2.5 text-xs font-light text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60"

  return (
    <Panel
      variant="soft"
      eyebrow="Canonical CRM"
      heading="Clients"
      subtitle={`${total.toLocaleString()} people in the canonical CRM — the primary relationship directory.`}
      flush
      divider
      action={
        <div className="flex min-h-9 flex-wrap items-center gap-2">
          <PortalInput
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search clients…"
            aria-label="Search clients"
            className="min-h-9 w-56 bg-white/70"
          />
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            aria-label="Filter by status"
            className={filterClass}
          >
            <option value="">All statuses</option>
            <option value="new">New</option>
            <option value="warm">Warm</option>
            <option value="active">Active</option>
            <option value="referral">Referral</option>
          </select>
          <select
            value={role}
            onChange={(e) => setRole(e.target.value)}
            aria-label="Filter by role"
            className={filterClass}
          >
            <option value="">All roles</option>
            <option value="buyer">Buyer</option>
            <option value="seller">Seller</option>
            <option value="both">Buyer &amp; Seller</option>
          </select>
          <select
            value={sort}
            onChange={(e) => setSort(e.target.value)}
            aria-label="Sort clients"
            className={filterClass}
          >
            <option value="name">Name (A–Z)</option>
            <option value="created">Newest</option>
            <option value="recent">Recently contacted</option>
          </select>
        </div>
      }
    >
      <div className="hidden md:block">
        <PortalTable>
          <PortalTableHead>
            <PortalTableRow className="hover:bg-transparent">
              <PortalTableHeader>Name</PortalTableHeader>
              <PortalTableHeader>Role / Status</PortalTableHeader>
              <PortalTableHeader>Email</PortalTableHeader>
              <PortalTableHeader>Phone</PortalTableHeader>
              <PortalTableHeader>Assigned</PortalTableHeader>
              <PortalTableHeader>Last Contact</PortalTableHeader>
              <PortalTableHeader>Sources</PortalTableHeader>
            </PortalTableRow>
          </PortalTableHead>
          <PortalTableBody>
            {rows.length > 0 ? (
              rows.map((row) => (
                <PortalTableRow key={row.id}>
                  <PortalTableCell>
                    <Link
                      href={`/portal/clients/${row.id}`}
                      className="font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                    >
                      {row.displayName}
                    </Link>
                    {row.location ? (
                      <div className="mt-1 text-xs font-light text-black/40">{row.location}</div>
                    ) : null}
                  </PortalTableCell>
                  <PortalTableCell>
                    <div className="text-sm font-medium">{roleLabel(row.role)}</div>
                    <div className="mt-1 text-xs font-light text-black/45">{statusLabel(row.status)}</div>
                  </PortalTableCell>
                  <PortalTableCell>
                    <div className="text-sm font-light">{row.primaryEmail ?? "—"}</div>
                  </PortalTableCell>
                  <PortalTableCell>
                    <div className="text-sm font-light">{row.primaryPhone ?? "—"}</div>
                  </PortalTableCell>
                  <PortalTableCell>
                    <div className="text-sm font-light">{row.assignedAgent ?? "—"}</div>
                  </PortalTableCell>
                  <PortalTableCell>
                    <div className="text-sm font-light">{row.lastContactLabel ?? "—"}</div>
                  </PortalTableCell>
                  <PortalTableCell>
                    <div className="text-[10px] font-light uppercase tracking-[0.08em] text-black/40">
                      {row.sources.length > 0 ? row.sources.join(", ") : "—"}
                    </div>
                  </PortalTableCell>
                </PortalTableRow>
              ))
            ) : (
              <PortalTableRow>
                <PortalTableCell colSpan={7} className="py-12 text-center text-black/45">
                  {loading ? "Loading clients…" : "No clients match."}
                </PortalTableCell>
              </PortalTableRow>
            )}
          </PortalTableBody>
        </PortalTable>
      </div>

      <div className="divide-y divide-[var(--portal-border)] md:hidden">
        {rows.length > 0 ? (
          rows.map((row) => (
            <article key={row.id} className="space-y-3 px-4 py-4">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <Link
                    href={`/portal/clients/${row.id}`}
                    className="font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                  >
                    {row.displayName}
                  </Link>
                  {row.location ? (
                    <p className="mt-0.5 truncate text-xs font-light text-black/40">{row.location}</p>
                  ) : null}
                </div>
                <span className="shrink-0 rounded-full border border-[var(--portal-border)] px-2 py-0.5 text-[9px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                  {roleLabel(row.role)} · {statusLabel(row.status)}
                </span>
              </div>
              <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                <div>
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Assigned</dt>
                  <dd className="mt-1 font-light text-black/65">{row.assignedAgent ?? "—"}</dd>
                </div>
                <div>
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Last Contact</dt>
                  <dd className="mt-1 font-light text-black/65">{row.lastContactLabel ?? "—"}</dd>
                </div>
                <div className="col-span-2">
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Email</dt>
                  <dd className="mt-1 break-words font-light text-black/65">{row.primaryEmail ?? "—"}</dd>
                </div>
                <div className="col-span-2">
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Phone</dt>
                  <dd className="mt-1 font-light text-black/65">{row.primaryPhone ?? "—"}</dd>
                </div>
              </dl>
            </article>
          ))
        ) : (
          <p className="px-4 py-10 text-center text-sm font-light text-black/45">
            {loading ? "Loading clients…" : "No clients match."}
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
          void load(debouncedSearch, next, status, role, sort)
        }}
        onNext={() => {
          const next = page + 1
          setPage(next)
          void load(debouncedSearch, next, status, role, sort)
        }}
      />
    </Panel>
  )
}

