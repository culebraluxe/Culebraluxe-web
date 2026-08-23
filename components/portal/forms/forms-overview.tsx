"use client"

import { useRouter } from "next/navigation"
import Link from "next/link"
import { useState, useTransition } from "react"

import { createFormAction } from "@/app/portal/forms/actions"
import { dateLabel } from "@/components/portal/storyboard/story-detail-sections"
import { Panel } from "@/components/portal/panel"

const selectClass =
  "min-h-11 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/50 px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
const labelClass =
  "text-[10px] font-light uppercase tracking-[0.18em] text-black/40"
const primaryButton =
  "inline-flex min-h-9 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[11px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"

export function FormsOverview({
  templates,
  deals,
  clients,
  properties,
  instances,
}: {
  templates: { id: string; displayName: string; version: number }[]
  deals: { id: string; label: string }[]
  clients: { id: string; label: string }[]
  properties: { id: string; label: string }[]
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
  const [templateId, setTemplateId] = useState(templates[0]?.id ?? "")
  const [dealId, setDealId] = useState("")
  const [personId, setPersonId] = useState("")
  const [propertyId, setPropertyId] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [isPending, startTransition] = useTransition()

  function createForm() {
    setError(null)
    startTransition(async () => {
      try {
        const result = await createFormAction({
          templateId,
          dealId: dealId || undefined,
          personId: personId || undefined,
          propertyId: propertyId || undefined,
        })
        if (result.ok) router.push(`/portal/forms/${result.data.formId}`)
        else setError(result.message ?? "Could not create the form.")
      } catch (caught) {
        setError(
          caught instanceof Error
            ? caught.message
            : "Could not create the form.",
        )
      }
    })
  }

  return (
    <div className="flex flex-col gap-4">
      <Panel compact heading="New form">
        <div className="grid gap-3 md:grid-cols-2">
          <label className="block">
            <span className={labelClass}>Form</span>
            <select
              value={templateId}
              onChange={(event) => setTemplateId(event.target.value)}
              className={`${selectClass} mt-2`}
            >
              {templates.map((template) => (
                <option key={template.id} value={template.id}>
                  {template.displayName}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className={labelClass}>Deal (optional)</span>
            <select
              value={dealId}
              onChange={(event) => setDealId(event.target.value)}
              className={`${selectClass} mt-2`}
            >
              <option value="">None — use client/property</option>
              {deals.map((deal) => (
                <option key={deal.id} value={deal.id}>
                  {deal.label}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className={labelClass}>Client (optional)</span>
            <select
              value={personId}
              onChange={(event) => setPersonId(event.target.value)}
              className={`${selectClass} mt-2`}
            >
              <option value="">—</option>
              {clients.map((client) => (
                <option key={client.id} value={client.id}>
                  {client.label}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className={labelClass}>Property (optional)</span>
            <select
              value={propertyId}
              onChange={(event) => setPropertyId(event.target.value)}
              className={`${selectClass} mt-2`}
            >
              <option value="">—</option>
              {properties.map((property) => (
                <option key={property.id} value={property.id}>
                  {property.label}
                </option>
              ))}
            </select>
          </label>
        </div>
        {error ? (
          <p className="mt-3 text-xs font-light text-[var(--portal-archive)]">
            {error}
          </p>
        ) : null}
        <button
          type="button"
          disabled={isPending || !templateId}
          onClick={createForm}
          className={`${primaryButton} mt-4`}
        >
          {isPending ? "Creating…" : "Start form"}
        </button>
      </Panel>

      <Panel compact heading="Drafts and issued" divider flush>
        <div className="overflow-x-auto">
          <table className="w-full border-collapse text-left text-sm">
            <thead>
              <tr className="border-b border-[var(--portal-panel-border)]">
                <th className="px-4 py-3 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                  Form
                </th>
                <th className="px-4 py-3 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                  Context
                </th>
                <th className="px-4 py-3 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                  Status
                </th>
                <th className="px-4 py-3 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                  Updated
                </th>
              </tr>
            </thead>
            <tbody>
              {instances.length === 0 ? (
                <tr>
                  <td
                    colSpan={4}
                    className="px-4 py-6 text-sm font-light text-black/40"
                  >
                    No forms yet.
                  </td>
                </tr>
              ) : (
                instances.map((instance) => (
                  <tr
                    key={instance.id}
                    className="border-b border-[var(--portal-panel-border)] last:border-b-0"
                  >
                    <td className="px-4 py-3">
                      <Link
                        href={`/portal/forms/${instance.id}`}
                        className="font-medium text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                      >
                        {templates.find((t) => t.id === instance.templateId)
                          ?.displayName ?? instance.templateId}
                      </Link>
                    </td>
                    <td className="px-4 py-3 font-light text-black/60">
                      {instance.clientName ?? instance.dealLabel ?? "—"}
                      {instance.propertyLabel ? (
                        <div className="text-xs text-black/40">
                          {instance.propertyLabel}
                        </div>
                      ) : null}
                    </td>
                    <td className="px-4 py-3 text-[10px] font-light uppercase tracking-[0.12em] text-black/45">
                      {instance.status}
                    </td>
                    <td className="px-4 py-3 text-xs font-light text-black/40">
                      {dateLabel(instance.updatedAt)}
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
