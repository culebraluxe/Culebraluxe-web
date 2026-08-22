"use client"

import Link from "next/link"
import { useState } from "react"

import { dateLabel } from "@/components/portal/storyboard/story-detail-sections"

const inputClass =
  "min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
const labelClass = "text-[10px] font-light uppercase tracking-[0.18em] text-black/40"

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

      <section className="rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="border-b border-[var(--portal-border)] px-6 py-6">
          <div className="flex flex-wrap items-end justify-between gap-4">
            <div>
              <h2 className="font-serif text-2xl font-light">Repository</h2>
              <p className="mt-1 text-sm font-light text-black/50">
                {documents.length} issued document
                {documents.length === 1 ? "" : "s"}
              </p>
            </div>
            <label className="block w-full max-w-xs">
              <span className={labelClass}>Filter by deal / client / property</span>
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Search…"
                className={`${inputClass} mt-2`}
              />
            </label>
          </div>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full border-collapse text-left text-sm">
            <thead>
              <tr className="border-b border-[var(--portal-border)]">
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">Document</th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">Deal / Client</th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">Version</th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">Issued</th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">By</th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">Status</th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">PDF</th>
              </tr>
            </thead>
// __PART2__
            <tbody>
              {visible.length === 0 ? (
                <tr className="border-b border-[var(--portal-border)]">
                  <td colSpan={7} className="px-6 py-6 text-sm font-light italic text-black/40">
                    No issued documents yet — assemble a form in NEXUS · Forms and issue it.
                  </td>
                </tr>
              ) : (
                visible.map((d) => (
                  <tr key={d.id} className="border-b border-[var(--portal-border)] last:border-b-0">
                    <td className="px-6 py-4">
                      <div className="font-light text-[var(--portal-navy)]">
                        {d.documentTypeLabel}
                      </div>
                      {d.title ? (
                        <div className="mt-0.5 text-xs font-light text-black/45">
                          {d.title}
                        </div>
                      ) : null}
                    </td>
                    <td className="px-6 py-4 font-light leading-5 text-black/60">
                      {d.partyName ?? d.dealName ?? "—"}
                      {d.propertyName ? (
                        <div className="text-xs text-black/40">{d.propertyName}</div>
                      ) : null}
                    </td>
                    <td className="px-6 py-4 font-light text-black/60">
                      {d.issuedVersion != null ? `v${d.issuedVersion}` : "—"}
                    </td>
                    <td className="px-6 py-4 text-xs font-light text-black/45">
                      {dateLabel(d.createdAt)}
                    </td>
                    <td className="px-6 py-4 text-xs font-light text-black/45">
                      {d.issuedByDisplayName ?? "—"}
                    </td>
                    <td className="px-6 py-4">
                      <span
                        className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${
                          d.state === "superseded"
                            ? "border border-black/10 text-black/35"
                            : "border border-[var(--portal-blue-gray)]/40 text-[var(--portal-navy-soft)]"
                        }`}
                      >
                        {d.state}
                      </span>
                    </td>
                    <td className="px-6 py-4">
                      <Link
                        href={`/portal/documents/${d.id}/download`}
                        className="text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)] underline underline-offset-4 hover:text-[#8a4b2a]"
                      >
                        Download
                      </Link>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  )
}

