"use client"

import { useRouter } from "next/navigation"
import Link from "next/link"
import { useState, useTransition } from "react"

import { createFormAction } from "@/app/portal/forms/actions"
import { dateLabel } from "@/components/portal/storyboard/story-detail-sections"

const selectClass =
  "mt-1 block h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-2.5 text-[13px] font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
const labelClass =
  "text-[9px] font-light uppercase tracking-[0.14em] text-black/40"
const primaryButton =
  "inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"

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
  const canStart = Boolean(templateId && (dealId || personId || propertyId))
  const selectedTemplate =
    templates.find((template) => template.id === templateId) ?? templates[0]

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
      <div>
        <h1 className="font-serif text-xl font-light text-[var(--portal-navy)]">
          {selectedTemplate?.displayName ?? "New form"}
        </h1>
        <p className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
          Create form
        </p>
      </div>

      <div className="grid items-start gap-4 lg:grid-cols-2">
        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
          <div className="grid grid-cols-6 gap-x-3 gap-y-3.5">
            <label className="col-span-6 min-w-0 sm:col-span-3">
              <span className={labelClass}>Form</span>
              <select
                value={templateId}
                onChange={(event) => setTemplateId(event.target.value)}
                className={selectClass}
              >
                {templates.map((template) => (
                  <option key={template.id} value={template.id}>
                    {template.displayName}
                  </option>
                ))}
              </select>
            </label>
            <label className="col-span-6 min-w-0 sm:col-span-3">
              <span className={labelClass}>Deal</span>
              <select
                value={dealId}
                onChange={(event) => setDealId(event.target.value)}
                className={selectClass}
              >
                <option value="">—</option>
                {deals.map((deal) => (
                  <option key={deal.id} value={deal.id}>
                    {deal.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="col-span-6 min-w-0 sm:col-span-3">
              <span className={labelClass}>Client</span>
              <select
                value={personId}
                onChange={(event) => setPersonId(event.target.value)}
                className={selectClass}
              >
                <option value="">—</option>
                {clients.map((client) => (
                  <option key={client.id} value={client.id}>
                    {client.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="col-span-6 min-w-0 sm:col-span-3">
              <span className={labelClass}>Property</span>
              <select
                value={propertyId}
                onChange={(event) => setPropertyId(event.target.value)}
                className={selectClass}
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

          <div className="mt-6">
            <h2 className="font-serif text-base font-bold text-[var(--portal-navy)]">
              Deal details
            </h2>
            <p className="mt-2 text-sm font-light leading-6 text-black/45">
              Pick a form and a deal, client, or property, then start. Fields
              and the document preview open on the next screen.
            </p>
          </div>

          <div className="mt-4 flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={isPending || !canStart}
              onClick={createForm}
              className={primaryButton}
            >
              {isPending ? "Opening…" : "Start"}
            </button>
          </div>

          {instances.length > 0 ? (
            <div className="mt-6 border-t border-[var(--portal-panel-border)] pt-4">
              <p className={labelClass}>Recent</p>
              <ul className="mt-2 space-y-1.5">
                {instances.slice(0, 8).map((instance) => (
                  <li key={instance.id}>
                    <Link
                      href={`/portal/forms/${instance.id}`}
                      className="block text-sm font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                    >
                      {templates.find((t) => t.id === instance.templateId)
                        ?.displayName ?? instance.templateId}
                      <span className="ml-2 text-xs text-black/40">
                        {instance.clientName ?? instance.dealLabel ?? "—"}
                        {" · "}
                        {dateLabel(instance.updatedAt)}
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </section>

        <section className="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)] lg:sticky lg:top-4">
          <div className="flex min-h-[70vh] flex-col bg-white px-8 py-7 text-black/80 lg:h-[calc(100vh-9.5rem)]">
            <p className="text-[9px] font-light uppercase tracking-[0.18em] text-black/40">
              CulebraLuxe Real Estate
            </p>
            <h2 className="mt-1 font-serif text-xl font-bold text-[var(--portal-navy)]">
              {selectedTemplate?.displayName ?? "Form"}
            </h2>
            <p className="mt-6 font-serif text-[15px] font-light leading-7 text-black/45">
              The live document preview will appear here after you start —
              filled fields on the left, this pane on the right.
            </p>
          </div>
        </section>
      </div>
    </div>
  )
}
