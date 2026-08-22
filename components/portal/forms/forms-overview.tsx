"use client"

import { useRouter } from "next/navigation"
import Link from "next/link"
import { useState, useTransition } from "react"

import { createOfferLetterFormAction } from "@/app/portal/forms/actions"
import { dateLabel } from "@/components/portal/storyboard/story-detail-sections"
import { Panel } from "@/components/portal/panel"

const selectClass =
  "min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
const labelClass = "text-[10px] font-light uppercase tracking-[0.18em] text-black/40"
const primaryButton =
  "inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"

export function FormsOverview({
  templates,
  deals,
  instances,
}: {
  templates: { id: string; displayName: string; version: number }[]
  deals: { id: string; label: string }[]
  instances: {
    id: string
    templateId: string
    status: string
    dealLabel: string | null
    clientName: string | null
    propertyLabel: string | null
    updatedAt: string
  }[]
}) {
  const router = useRouter()
  const [selectedDeal, setSelectedDeal] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [isPending, startTransition] = useTransition()

  const template = templates[0]

  function createForm() {
    setError(null)
    if (!selectedDeal) {
      setError("Select a deal to assemble the Offer Letter for.")
      return
    }
    startTransition(async () => {
      const result = await createOfferLetterFormAction(selectedDeal)
      if (result.ok) {
        router.push(`/portal/forms/${result.data.formId}`)
      } else {
        setError(result.message ?? "Could not create the form.")
      }
    })
  }

  return (
    <div>
      <header className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          NEXUS
        </p>
        <div className="mt-3 flex items-baseline gap-4">
          <h1 className="font-serif text-4xl font-light leading-[1.1]">
            Forms
          </h1>
          <span className="rounded-full bg-[var(--portal-blue-pale)] px-3 py-1 text-xs font-light uppercase tracking-[0.16em] text-[var(--portal-navy-soft)]">
            POC — {template ? `${template.displayName} v${template.version}` : "Offer Letter"}
          </span>
        </div>
        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Approved transaction forms with structured fields and bounded editable
          sections. Drafts are mutable working state; issuing hands an immutable
          PDF to the Documents repository.
        </p>
      </header>

      <Panel
        variant="standard"
        heading="New Offer Letter"
        subtitle="Choose the deal; known client, property and deal facts are prefilled automatically."
        divider
        flush
      >
        <div className="px-6 py-6">
          <label className="block max-w-md">
            <span className={labelClass}>Deal</span>
            <select
              value={selectedDeal}
              onChange={(event) => setSelectedDeal(event.target.value)}
              className={`${selectClass} mt-2`}
            >
              <option value="">Select a deal…</option>
              {deals.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.label}
                </option>
              ))}
            </select>
          </label>
          {error && (
            <p className="mt-3 text-xs font-light text-[var(--portal-archive)]">{error}</p>
          )}
          <button
            type="button"
            disabled={isPending || deals.length === 0}
            onClick={createForm}
            className={`${primaryButton} mt-4`}
          >
            {isPending ? "Creating…" : "Create form"}
          </button>
        </div>
      </Panel>
// __PART2__
      <Panel
        variant="standard"
        heading="Form instances"
        subtitle="Mutable working drafts. A draft becomes immutable once issued."
        divider
        flush
      >
        <div className="overflow-x-auto">
          <table className="w-full border-collapse text-left text-sm">
            <thead>
              <tr className="border-b border-[var(--portal-border)]">
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Template
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Deal / Client
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Status
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Updated
                </th>
              </tr>
            </thead>
            <tbody>
              {instances.length === 0 ? (
                <tr className="border-b border-[var(--portal-border)]">
                  <td
                    colSpan={4}
                    className="px-6 py-6 text-sm font-light italic text-black/40"
                  >
                    No form instances yet — create your first Offer Letter above.
                  </td>
                </tr>
              ) : (
                instances.map((f) => (
                  <tr key={f.id} className="border-b border-[var(--portal-border)] last:border-b-0">
                    <td className="px-6 py-4">
                      <Link
                        href={`/portal/forms/${f.id}`}
                        className="font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-archive)]"
                      >
                        {f.templateId}
                      </Link>
                    </td>
                    <td className="px-6 py-4 font-light leading-5 text-black/60">
                      {f.clientName ?? f.dealLabel ?? "—"}
                      {f.propertyLabel ? (
                        <div className="text-xs text-black/40">{f.propertyLabel}</div>
                      ) : null}
                    </td>
                    <td className="px-6 py-4">
                      <span
                        className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${
                          f.status === "issued"
                            ? "border border-[var(--portal-blue-gray)]/40 text-[var(--portal-navy-soft)]"
                            : "border border-black/10 text-black/45"
                        }`}
                      >
                        {f.status}
                      </span>
                    </td>
                    <td className="px-6 py-4 text-xs font-light text-black/45">
                      {dateLabel(f.updatedAt)}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </Panel>
    </div>
  )
}

