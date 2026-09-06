"use client"

import { useEffect, useMemo } from "react"
import { Home, MapPin, UserRound } from "lucide-react"

import { Panel } from "@/components/portal/panel"
import { formatPhone, roleLabel, statusDot } from "@/components/portal/client-display"
import type { ClientRole } from "@/lib/portal/types"
import type { TemplateDefinition } from "@/lib/forms/template-types"
import {
  FormLensController,
  HttpFormLensSource,
  formatPropertyAddress,
  type FormLensFieldModel,
  type FormLensFieldOrigin,
  type FormLensSource,
} from "@/ui/form-lens"
import { usePageController } from "@/ui/runtime"

function originLabel(origin: FormLensFieldOrigin): string {
  switch (origin) {
    case "person": return "Person"
    case "property": return "Property"
    case "property_relation": return "Property relation"
    case "template_default": return "Template default"
    case "manual": return "Local edit"
    default: return "Needs input"
  }
}

function originClass(origin: FormLensFieldOrigin): string {
  if (origin === "unresolved") return "text-[var(--portal-archive)]"
  if (origin === "manual") return "text-[var(--portal-gold-muted)]"
  return "text-[var(--portal-navy-soft)]"
}

const fieldClass =
  "mt-1 block min-h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-2.5 text-[13px] font-light text-black/70 outline-none focus:border-[var(--portal-navy)]"

function DraftField({
  field,
  onChange,
}: {
  field: FormLensFieldModel
  onChange: (value: string) => void
}) {
  return (
    <label className="block">
      <div className="flex items-center justify-between gap-3">
        <span className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
          {field.label}{field.required ? " *" : ""}
        </span>
        <span className={`text-[9px] font-light uppercase tracking-[0.1em] ${originClass(field.origin)}`}>
          {originLabel(field.origin)}
        </span>
      </div>

      {field.type === "select" ? (
        <select
          value={field.value}
          onChange={(event) => onChange(event.target.value)}
          className={fieldClass}
        >
          <option value="">Select…</option>
          {field.options.map((option) => (
            <option key={option} value={option}>{option}</option>
          ))}
        </select>
      ) : field.type === "textarea" ? (
        <textarea
          value={field.value}
          onChange={(event) => onChange(event.target.value)}
          className={`${fieldClass} min-h-24 py-2`}
        />
      ) : (
        <input
          type={field.type === "date" ? "date" : "text"}
          inputMode={field.type === "money" ? "decimal" : undefined}
          value={field.value}
          onChange={(event) => onChange(event.target.value)}
          className={fieldClass}
        />
      )}
    </label>
  )
}

export function FormLens({
  template,
  source,
}: {
  template: TemplateDefinition
  source?: FormLensSource
}) {
  const controller = useMemo(
    () => new FormLensController(source ?? new HttpFormLensSource(), template),
    [source, template],
  )
  const model = usePageController(controller)

  useEffect(() => {
    void controller.dispatch({ operation: "formLens.load", payload: {} })
    return () => controller.dispose()
  }, [controller])

  const context = model.propertyContext
  const canonicalProperties = context?.properties ?? []
  const observedAddresses = context?.observedAddresses ?? []

  return (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="flex items-center justify-between gap-4 px-1">
        <div>
          <div className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
            Architecture Sidecar
          </div>
          <h1 className="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">
            Form Lens · Listing Agreement
          </h1>
        </div>
        <div className="text-right text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
          Person → Property → LISTING-01 v{template.version}<br />
          local draft only · no writes
        </div>
      </div>

      <div className="grid min-h-0 flex-1 gap-4 lg:h-[calc(100dvh-10.5rem)] lg:grid-cols-[220px_minmax(280px,0.9fr)_minmax(360px,1.2fr)]">
        <aside className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="shrink-0 border-b border-[var(--portal-panel-border)] p-3">
            <div className="mb-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
              Seller / Person · {model.total.toLocaleString()}
            </div>
            <input
              type="search"
              value={model.query}
              onChange={(event) => {
                void controller.dispatch({
                  operation: "formLens.queryChanged",
                  payload: { query: event.target.value },
                })
              }}
              placeholder="Search people…"
              className="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-2.5 py-2 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
            />
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto">
            {model.list.length === 0 ? (
              <p className="px-3 py-6 text-sm font-light text-black/40">
                {model.listLoading ? "Loading…" : model.listError ?? "No matching people."}
              </p>
            ) : (
              model.list.map((item) => {
                const selected = model.selectedPersonId === item.id
                return (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => {
                      void controller.dispatch({
                        operation: "formLens.selectPerson",
                        payload: { personId: item.id },
                      })
                    }}
                    className={[
                      "flex w-full items-start gap-2 border-b border-[var(--portal-panel-border)] px-3 py-2.5 text-left transition",
                      selected
                        ? "border-l-2 border-l-[var(--portal-gold)] bg-white/45"
                        : "border-l-2 border-l-transparent hover:bg-white/25",
                    ].join(" ")}
                  >
                    <span className={`mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full ${statusDot(item.status)}`} />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-medium text-[var(--portal-navy)]">
                        {item.nameResolved ? item.displayName : "Unknown contact"}
                      </div>
                      <div className="truncate text-[11px] font-light text-black/45">
                        {formatPhone(item.primaryPhone) ?? item.primaryEmail ?? roleLabel(item.role as ClientRole)}
                      </div>
                    </div>
                  </button>
                )
              })
            )}
          </div>

          <div className="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-2">
            <button
              type="button"
              disabled={model.page <= 1}
              onClick={() => void controller.dispatch({ operation: "formLens.previousPage", payload: {} })}
              className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30"
            >
              ← Prev
            </button>
            <span className="text-[10px] font-light text-black/40">{model.page} / {model.pageCount}</span>
            <button
              type="button"
              disabled={model.page >= model.pageCount}
              onClick={() => void controller.dispatch({ operation: "formLens.nextPage", payload: {} })}
              className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30"
            >
              Next →
            </button>
          </div>
        </aside>

        <main className="min-h-0 space-y-4 overflow-y-auto">
          <Panel compact heading="Seller Context">
            {model.clientLoading ? (
              <p className="text-sm font-light text-black/45">Loading Person…</p>
            ) : model.clientError ? (
              <p className="text-sm font-light text-[var(--portal-archive)]">{model.clientError}</p>
            ) : model.client ? (
              <div className="flex items-start gap-3">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-[var(--portal-panel-border)] bg-white/50">
                  <UserRound className="h-4 w-4 text-[var(--portal-navy-soft)]" aria-hidden />
                </div>
                <div className="min-w-0">
                  <div className="font-serif text-xl font-light text-[var(--portal-navy)]">
                    {model.client.displayName}
                  </div>
                  <div className="mt-1 text-xs font-light text-black/50">
                    {model.client.email ?? "No email"} · {formatPhone(model.client.phone) ?? "No phone"}
                  </div>
                </div>
              </div>
            ) : (
              <p className="text-sm font-light text-black/45">Select a Person.</p>
            )}
          </Panel>

          <Panel
            compact
            heading="Property Context"
            action={
              <span className="text-[9px] font-light uppercase tracking-[0.1em] text-black/35">
                {model.propertyLoading ? "Loading…" : `${canonicalProperties.length} canonical · ${observedAddresses.length} observed`}
              </span>
            }
          >
            {model.propertyLoading ? (
              <p className="text-sm font-light text-black/45">Loading Property service…</p>
            ) : model.propertyError ? (
              <p className="text-sm font-light text-[var(--portal-archive)]">{model.propertyError}</p>
            ) : canonicalProperties.length === 0 ? (
              <p className="text-sm font-light text-black/45">
                No canonical Property is linked to this Person. Apple observations stay evidence until promoted.
              </p>
            ) : (
              <div className="space-y-2">
                {canonicalProperties.map(({ property, relation }) => {
                  const selected = property.id === model.selectedPropertyId
                  return (
                    <button
                      key={`${property.id}:${relation}`}
                      type="button"
                      onClick={() => {
                        void controller.dispatch({
                          operation: "formLens.selectProperty",
                          payload: { propertyId: property.id },
                        })
                      }}
                      className={[
                        "w-full rounded-[var(--portal-tab-radius)] border px-3 py-2.5 text-left transition",
                        selected
                          ? "border-[var(--portal-gold)] bg-white/65"
                          : "border-[var(--portal-panel-border)] bg-white/35 hover:bg-white/50",
                      ].join(" ")}
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex items-center gap-2">
                            <Home className="h-3.5 w-3.5 shrink-0 text-[var(--portal-navy-soft)]" aria-hidden />
                            <span className="truncate font-serif text-base font-light text-[var(--portal-navy)]">
                              {property.localName ?? property.displayName}
                            </span>
                          </div>
                          <div className="mt-1 pl-5 text-xs font-light leading-5 text-black/55">
                            {formatPropertyAddress(property.address) || "Structured address incomplete"}
                          </div>
                          {property.legalOwnerName ? (
                            <div className="mt-1 pl-5 text-[10px] font-light text-black/40">
                              Legal owner: {property.legalOwnerName}
                            </div>
                          ) : null}
                        </div>
                        <span className="shrink-0 rounded-full bg-[var(--portal-blue-pale)] px-2 py-1 text-[9px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                          {relation.replaceAll("_", " ")}
                        </span>
                      </div>
                    </button>
                  )
                })}
              </div>
            )}

            {observedAddresses.length > 0 ? (
              <div className="mt-4 border-t border-[var(--portal-panel-border)] pt-3">
                <div className="mb-2 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                  Address evidence · not auto-filled
                </div>
                <div className="space-y-2">
                  {observedAddresses.map((observation) => (
                    <div key={observation.sourceKey} className="rounded-[var(--portal-tab-radius)] bg-white/30 px-3 py-2">
                      <div className="flex items-start gap-2">
                        <MapPin className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--portal-gold-muted)]" aria-hidden />
                        <div className="min-w-0">
                          <div className="text-xs font-light text-black/65">
                            {formatPropertyAddress(observation.address) || "Empty address observation"}
                          </div>
                          <div className="mt-0.5 text-[9px] font-light uppercase tracking-[0.1em] text-black/35">
                            {observation.sourceLabel || "Apple Contacts"} · {observation.matchedPropertyId ? "matched" : "candidate"}
                          </div>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ) : null}
          </Panel>
        </main>

        <section className="portal-glass-panel min-h-0 overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="flex items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-4 py-3">
            <div>
              <div className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
                {template.id} · v{template.version}
              </div>
              <div className="mt-0.5 font-serif text-lg font-light text-[var(--portal-navy)]">
                {template.displayName}
              </div>
            </div>
            <button
              type="button"
              disabled={model.manualFields.length === 0}
              onClick={() => void controller.dispatch({ operation: "formLens.resetDraft", payload: {} })}
              className="text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30"
            >
              Reset hydration
            </button>
          </div>

          <div className="h-full overflow-y-auto px-4 py-4 pb-24">
            <div className="mb-4 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 px-3 py-2 text-[10px] font-light leading-4 text-black/45">
              This is the new composition seam only. Person and Property hydrate the XML form contract; edits stay local. Existing Forms, issued documents, signatures, Deal, and Production are untouched.
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              {model.fields.map((field) => (
                <div
                  key={field.name}
                  className={field.type === "textarea" ? "sm:col-span-2" : undefined}
                >
                  <DraftField
                    field={field}
                    onChange={(value) => {
                      void controller.dispatch({
                        operation: "formLens.fieldChanged",
                        payload: { name: field.name, value },
                      })
                    }}
                  />
                </div>
              ))}
            </div>
          </div>
        </section>
      </div>
    </div>
  )
}
