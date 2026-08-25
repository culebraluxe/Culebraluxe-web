"use client"

import Link from "next/link"
import { useState } from "react"

import { dateLabel } from "@/components/portal/storyboard/story-detail-sections"
import { Panel } from "@/components/portal/panel"
import {
  PortalField,
  PortalFieldLabel,
  PortalInput,
} from "@/components/portal/ui/portal-field"
import {
  PortalTable,
  PortalTableBody,
  PortalTableCell,
  PortalTableHead,
  PortalTableHeader,
  PortalTableRow,
} from "@/components/portal/ui/portal-table"

export function DocumentList({
  documents,
}: {
  documents: {
    id: string
    documentTypeLabel: string
    title: string | null
    state: string
    templateId: string | null
    issuedVersion: number | null
    issuedChecksumSha256: string | null
    issuedByDisplayName: string | null
    partyName: string | null
    propertyName: string | null
    dealName: string | null
    createdAt: string
  }[]
}) {
  const [query, setQuery] = useState("")

  const needle = query.trim().toLowerCase()
  const visible = needle
    ? documents.filter((d) =>
        [
          d.dealName,
          d.partyName,
          d.propertyName,
          d.documentTypeLabel,
          d.issuedByDisplayName,
        ]
          .filter(Boolean)
          .some((value) => String(value).toLowerCase().includes(needle)),
      )
    : documents

  return (
    <div>
      <header className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          NEXUS
        </p>
        <div className="mt-3 flex items-baseline gap-4">
          <h1 className="font-serif text-4xl font-light leading-[1.1]">
            Documents
          </h1>
          <span className="rounded-full bg-[var(--portal-blue-gray)]/15 px-3 py-1 text-xs font-light uppercase tracking-[0.16em] text-[var(--portal-navy-soft)]">
            Issued repository
          </span>
        </div>
        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Canonical issued artifacts only. Retrieval is by deal, client,
          property or document type — never folders. Issued documents are
          immutable; a revised issuance is a new version, not an overwrite.
        </p>
      </header>

      <Panel
        variant="standard"
        heading="Repository"
        subtitle={`${documents.length} issued document${documents.length === 1 ? "" : "s"}`}
        action={
          <PortalField className="w-full max-w-xs">
            <PortalFieldLabel>Filter by deal / client / property</PortalFieldLabel>
            <PortalInput
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search…"
            />
          </PortalField>
        }
        divider
        flush
      >
        <div className="hidden md:block">
          <PortalTable>
            <PortalTableHead>
              <PortalTableRow className="hover:bg-transparent">
                <PortalTableHeader>Document</PortalTableHeader>
                <PortalTableHeader>Deal / Client</PortalTableHeader>
                <PortalTableHeader>Version</PortalTableHeader>
                <PortalTableHeader>Issued</PortalTableHeader>
                <PortalTableHeader>By</PortalTableHeader>
                <PortalTableHeader>Status</PortalTableHeader>
                <PortalTableHeader>PDF</PortalTableHeader>
              </PortalTableRow>
            </PortalTableHead>
            <PortalTableBody>
              {visible.length === 0 ? (
                <PortalTableRow>
                  <PortalTableCell colSpan={7} className="py-6 italic text-black/40">
                    No issued documents yet — assemble a form in NEXUS · Forms and issue it.
                  </PortalTableCell>
                </PortalTableRow>
              ) : (
                visible.map((d) => (
                  <PortalTableRow key={d.id}>
                    <PortalTableCell>
                      <div className="font-light text-[var(--portal-navy)]">
                        {d.documentTypeLabel}
                      </div>
                      {d.title ? (
                        <div className="mt-0.5 text-xs font-light text-black/45">
                          {d.title}
                        </div>
                      ) : null}
                    </PortalTableCell>
                    <PortalTableCell className="font-light leading-5 text-black/60">
                      {d.partyName ?? d.dealName ?? "—"}
                      {d.propertyName ? (
                        <div className="text-xs text-black/40">{d.propertyName}</div>
                      ) : null}
                    </PortalTableCell>
                    <PortalTableCell className="font-light text-black/60">
                      {d.issuedVersion != null ? `v${d.issuedVersion}` : "—"}
                    </PortalTableCell>
                    <PortalTableCell className="text-xs font-light text-black/45">
                      {dateLabel(d.createdAt)}
                    </PortalTableCell>
                    <PortalTableCell className="text-xs font-light text-black/45">
                      {d.issuedByDisplayName ?? "—"}
                    </PortalTableCell>
                    <PortalTableCell>
                      <span
                        className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${
                          d.state === "superseded"
                            ? "border border-black/10 text-black/35"
                            : "border border-[var(--portal-blue-gray)]/40 text-[var(--portal-navy-soft)]"
                        }`}
                      >
                        {d.state}
                      </span>
                    </PortalTableCell>
                    <PortalTableCell>
                      <Link
                        href={`/portal/documents/${d.id}/download`}
                        className="text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)] underline underline-offset-4 hover:text-[var(--portal-archive)]"
                      >
                        Download
                      </Link>
                    </PortalTableCell>
                  </PortalTableRow>
                ))
              )}
            </PortalTableBody>
          </PortalTable>
        </div>

        <div className="divide-y divide-[var(--portal-border)] md:hidden">
          {visible.length === 0 ? (
            <p className="px-4 py-6 text-sm font-light italic text-black/40">
              No issued documents yet — assemble a form in NEXUS · Forms and issue it.
            </p>
          ) : (
            visible.map((d) => (
              <article key={d.id} className="space-y-3 px-4 py-4">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <h3 className="font-light text-[var(--portal-navy)]">{d.documentTypeLabel}</h3>
                    {d.title ? (
                      <p className="mt-0.5 text-xs font-light text-black/45">{d.title}</p>
                    ) : null}
                  </div>
                  <span
                    className={`inline-block shrink-0 whitespace-nowrap rounded-full px-3 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${
                      d.state === "superseded"
                        ? "border border-black/10 text-black/35"
                        : "border border-[var(--portal-blue-gray)]/40 text-[var(--portal-navy-soft)]"
                    }`}
                  >
                    {d.state}
                  </span>
                </div>
                <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                  <div className="col-span-2">
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Deal / Client</dt>
                    <dd className="mt-1 font-light text-black/65">
                      {d.partyName ?? d.dealName ?? "—"}
                      {d.propertyName ? <span className="text-black/40"> · {d.propertyName}</span> : null}
                    </dd>
                  </div>
                  <div>
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Version</dt>
                    <dd className="mt-1 font-light text-black/65">
                      {d.issuedVersion != null ? `v${d.issuedVersion}` : "—"}
                    </dd>
                  </div>
                  <div>
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Issued</dt>
                    <dd className="mt-1 font-light text-black/65">{dateLabel(d.createdAt)}</dd>
                  </div>
                  <div className="col-span-2">
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">By</dt>
                    <dd className="mt-1 font-light text-black/65">{d.issuedByDisplayName ?? "—"}</dd>
                  </div>
                </dl>
                <div>
                  <Link
                    href={`/portal/documents/${d.id}/download`}
                    className="inline-flex min-h-11 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-archive)]"
                  >
                    Download
                  </Link>
                </div>
              </article>
            ))
          )}
        </div>
      </Panel>
    </div>
  )
}

