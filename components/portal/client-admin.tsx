"use client"

import Link from "next/link"
import { useCallback, useEffect, useState } from "react"

import type { ClientAdminRow, ClientAdminPageResult } from "@/db/client-admin"
import { ClientArchiveButton } from "@/components/portal/write/client-archive-button"
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

function roleLabel(role: string) {
  if (role === "both") return "Buyer & Seller"
  return role.charAt(0).toUpperCase() + role.slice(1)
}

function statusLabel(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

// Client Administration is a server-side-paginated read over the canonical
// `person` parent. It fetches its own pages from /api/portal/clients?view=admin
// so it never materializes the whole person table.
const PAGE_SIZE = 50

export function ClientAdmin() {
  const [search, setSearch] = useState("")
  const [debouncedSearch, setDebouncedSearch] = useState("")
  const [page, setPage] = useState(1)
  const [rows, setRows] = useState<ClientAdminRow[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const id = setTimeout(() => setDebouncedSearch(search), 250)
    return () => clearTimeout(id)
  }, [search])

  const load = useCallback(async (q: string, p: number) => {
    setLoading(true)
    const params = new URLSearchParams({
      view: "admin",
      search: q,
      page: String(p),
      pageSize: String(PAGE_SIZE),
    })
    try {
      const res = await fetch(`/api/portal/clients?${params.toString()}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = (await res.json()) as ClientAdminPageResult
      setRows(json.rows)
      setTotal(json.total)
    } catch (err) {
      console.error("Failed to load client administration:", err)
      setRows([])
      setTotal(0)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    setPage(1)
    void load(debouncedSearch, 1)
  }, [debouncedSearch, load])

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))

  return (
    <section className="mt-10 overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel">
      <div className="flex items-center justify-between gap-3 border-b border-[var(--portal-border)] px-6 py-5">
        <div>
          <h2 className="font-serif text-2xl font-light">Client Administration</h2>
          <p className="mt-1 text-xs font-light text-black/40">
            Read-only operational view of people, contact coverage, and activity.
          </p>
        </div>
        <div className="flex min-h-9 items-center gap-3">
          <PortalInput
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search people…"
            aria-label="Search client administration"
            className="min-h-9 w-52 bg-white/70"
          />
          <span className="text-xs font-light text-black/35">{total.toLocaleString()} people</span>
        </div>
      </div>

      <div className="hidden md:block">
        <PortalTable>
          <PortalTableHead>
            <PortalTableRow className="hover:bg-transparent">
              <PortalTableHeader>Person</PortalTableHeader>
              <PortalTableHeader>Role / Status</PortalTableHeader>
              <PortalTableHeader>Email</PortalTableHeader>
              <PortalTableHeader>Phone</PortalTableHeader>
              <PortalTableHeader>Assigned</PortalTableHeader>
              <PortalTableHeader>Last Interaction</PortalTableHeader>
              <PortalTableHeader>Open Tasks</PortalTableHeader>
              <PortalTableHeader>Active Deals</PortalTableHeader>
              <PortalTableHeader>Interests</PortalTableHeader>
              <PortalTableHeader>Actions</PortalTableHeader>
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
                    {row.location && (
                      <div className="mt-1 text-xs font-light text-black/40">
                        {row.location}
                      </div>
                    )}
                  </PortalTableCell>
                  <PortalTableCell>
                    <div className="text-sm font-medium">{roleLabel(row.role)}</div>
                    <div className="mt-1 text-xs font-light text-black/45">
                      {statusLabel(row.status)}
                    </div>
                  </PortalTableCell>
                  <PortalTableCell>
                    <div className="text-sm font-light">
                      {row.primaryEmail ?? "—"}
                    </div>
                    {!row.primaryEmail && (
                      <div className="mt-1 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-archive)]">
                        No email
                      </div>
                    )}
                  </PortalTableCell>
                  <PortalTableCell>
                    <div className="text-sm font-light">{row.primaryPhone ?? "—"}</div>
                    {!row.primaryPhone && (
                      <div className="mt-1 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-archive)]">
                        No phone
                      </div>
                    )}
                  </PortalTableCell>
                  <PortalTableCell className="text-sm font-light">
                    {row.assignedAgent ?? "—"}
                  </PortalTableCell>
                  <PortalTableCell className="text-sm font-light">
                    {row.lastInteractionLabel ?? "—"}
                  </PortalTableCell>
                  <PortalTableCell className="text-sm font-light">
                    {row.openTaskCount}
                  </PortalTableCell>
                  <PortalTableCell className="text-sm font-light">
                    {row.activeDealCount}
                  </PortalTableCell>
                  <PortalTableCell className="text-sm font-light">
                    {row.interestCount}
                  </PortalTableCell>
                  <PortalTableCell>
                    <ClientArchiveButton
                      personId={row.id}
                      displayName={row.displayName}
                    />
                  </PortalTableCell>
                </PortalTableRow>
              ))
            ) : (
              <PortalTableRow>
                <PortalTableCell
                  colSpan={10}
                  className="py-12 text-center text-black/40"
                >
                  No people on file.
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
                  {row.location && (
                    <p className="mt-0.5 truncate text-xs font-light text-black/40">
                      {row.location}
                    </p>
                  )}
                </div>
                <ClientArchiveButton
                  personId={row.id}
                  displayName={row.displayName}
                />
              </div>
              <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                <div>
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Role / Status</dt>
                  <dd className="mt-1 font-light text-black/65">
                    {roleLabel(row.role)} · {statusLabel(row.status)}
                  </dd>
                </div>
                <div>
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Assigned</dt>
                  <dd className="mt-1 font-light text-black/65">{row.assignedAgent ?? "—"}</dd>
                </div>
                <div className="col-span-2">
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Email</dt>
                  <dd className="mt-1 break-words font-light text-black/65">
                    {row.primaryEmail ?? "—"}
                  </dd>
                </div>
                <div className="col-span-2">
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Phone</dt>
                  <dd className="mt-1 font-light text-black/65">{row.primaryPhone ?? "—"}</dd>
                </div>
                <div>
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Last Interaction</dt>
                  <dd className="mt-1 font-light text-black/65">{row.lastInteractionLabel ?? "—"}</dd>
                </div>
                <div>
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Tasks</dt>
                  <dd className="mt-1 font-light text-black/65">{row.openTaskCount}</dd>
                </div>
                <div>
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Active Deals</dt>
                  <dd className="mt-1 font-light text-black/65">{row.activeDealCount}</dd>
                </div>
                <div>
                  <dt className="font-light uppercase tracking-[0.12em] text-black/35">Interests</dt>
                  <dd className="mt-1 font-light text-black/65">{row.interestCount}</dd>
                </div>
              </dl>
            </article>
          ))
        ) : (
          <p className="px-4 py-10 text-center text-sm font-light text-black/40">
            {loading ? "Loading people…" : "No people on file."}
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
    </section>
  )
}
