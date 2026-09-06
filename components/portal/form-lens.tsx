"use client"

import { useEffect, useMemo } from "react"
import { Home, MapPin, Save, UserRound } from "lucide-react"

import { Panel } from "@/components/portal/panel"
import { formatPhone, roleLabel, statusDot } from "@/components/portal/client-display"
import { LISTING_CANONICAL_FIELD_NAMES } from "@/lib/forms/listing-field-binding"
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

const CANONICAL_FIELDS = new Set<string>(LISTING_CANONICAL_FIELD_NAMES)

function originLabel(origin: FormLensFieldOrigin): string {
  switch (origin) {
    case "person": return "Person"
    case "property": return "Property"
    case "property_relation": return "Property relation"
    case "listing_form": return "Listing evidence"
    case "template_default": return "Template default"
    case "manual": return "Edited"
    default: return "Needs input"
  }
}

function originClass(origin: FormLensFieldOrigin): string {
  if (origin === "unresolved") return "text-[var(--portal-archive)]"
  if (origin === "manual" || origin === "listing_form") return "text-[var(--portal-gold-muted)]"
  return "text-[var(--portal-navy-soft)]"
}

const fieldClass =
  "mt-1 block min-h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-2.5 text-[13px] font-light text-black/70 outline-none focus:border-[var(--portal-navy)]"

function DraftField({ field, onChange }: { field: FormLensFieldModel; onChange: (value: string) => void }) {
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
        <select value={field.value} onChange={(event) => onChange(event.target.value)} className={fieldClass}>
          <option value="">Select…</option>
          {field.options.map((option) => <option key={option} value={option}>{option}</option>)}
        </select>
      ) : field.type === "textarea" ? (
        <textarea value={field.value} onChange={(event) => onChange(event.target.value)} className={`${fieldClass} min-h-24 py-2`} />
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

export function FormLens({ template, source }: { template: TemplateDefinition; source?: FormLensSource }) {
  const controller = useMemo(
    () => new FormLensController(source ?? new HttpFormLensSource(), template),
    [source, template],
  )
  const model = usePageController(controller)

  useEffect(() => {
    void controller.dispatch({ operation: "formLens.load", payload: {} })
    // Deliberately do not dispose the memoized controller in ordinary effect
    // cleanup; React StrictMode runs cleanup/setup twice in development.
  }, [controller])

  const context = model.propertyContext
  const canonicalProperties = context?.properties ?? []
  const observedAddresses = context?.observedAddresses ?? []
  const evidenceFields = model.fields.filter((field) => field.origin === "listing_form")
  const promotable = model.fields.some((field) =>
    CANONICAL_FIELDS.has(field.name) &&
    (field.origin === "listing_form" || model.manualFields.includes(field.name)),
  )

  return (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="flex items-center justify-between gap-4 px-1">
        <div>
          <div className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">Architecture Sidecar</div>
          <h1 className="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">Form Lens · Listing Agreement</h1>
        </div>
        <div className="text-right text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
          Person ↔ Property ↔ LISTING-01 v{template.version}<br />
          canonical first · form fallback · reviewed write-back
        </div>
      </div>

      <div className="grid min-h-0 flex-1 gap-4 lg:h-[calc(100dvh-10.5rem)] lg:grid-cols-[220px_minmax(280px,0.9fr)_minmax(360px,1.2fr)]">
        <aside className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="shrink-0 border-b border-[var(--portal-panel-border)] p-3">
            <div className="mb-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">Seller / Person · {model.total.toLocaleString()}</div>
            <input
              type="search"
              value={model.query}
              onChange={(event) => void controller.dispatch({ operation: "formLens.queryChanged", payload: { query: event.target.value } })}
              placeholder="Search Jessica…"
              className="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-2.5 py-2 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
            />
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {model.list.length === 0 ? (
              <p className="px-3 py-6 text-sm font-light text-black/40">{model.listLoading ? "Loading…" : model.listError ?? "No matching people."}</p>
            ) : model.list.map((item) => {
              const selected = model.selectedPersonId === item.id
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => void controller.dispatch({ operation: "formLens.selectPerson", payload: { personId: item.id } })}
                  className={[
                    "flex w-full items-start gap-2 border-b border-[var(--portal-panel-border)] px-3 py-2.5 text-left transition",
                    selected ? "border-l-2 border-l-[var(--portal-gold)] bg-white/45" : "border-l-2 border-l-transparent hover:bg-white/25",
                  ].join(" ")}
                >
                  <span className={`mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full ${statusDot(item.status)}`} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] font-medium text-[var(--portal-navy)]">{item.nameResolved ? item.displayName : "Unknown contact"}</div>
                    <div className="truncate text-[11px] font-light text-black/45">
                      {formatPhone(item.primaryPhone) ?? item.primaryEmail ?? roleLabel(item.role as ClientRole)}
                    </div>
                  </div>
                </button>
              )
            })}
          </div>
          <div className="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-2">
            <button type="button" disabled={model.page <= 1} onClick={() => void controller.dispatch({ operation: "formLens.previousPage", payload: {} })} className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">← Prev</button>
            <span className="text-[10px] font-light text-black/40">{model.page} / {model.pageCount}</span>
            <button type="button" disabled={model.page >= model.pageCount} onClick={() => void controller.dispatch({ operation: "formLens.nextPage", payload: {} })} className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">Next →</button>
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
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-[var(--portal-panel-border)] bg-white/50"><UserRound className="h-4 w-4 text-[var(--portal-navy-soft)]" aria-hidden /></div>
                <div>
                  <div className="font-serif text-xl font-light text-[var(--portal-navy)]">{model.client.displayName}</div>
                  <div className="mt-1 text-xs font-light text-black/50">{model.client.email ?? "No email"} · {formatPhone(model.client.phone) ?? "No phone"}</div>
                </div>
              </div>
            ) : <p className="text-sm font-light text-black/45">Select a Person.</p>}
          </Panel>

          <Panel
            compact
            heading="Property Context"
            action={<span className="text-[9px] font-light uppercase tracking-[0.1em] text-black/35">{model.propertyLoading ? "Loading…" : `${canonicalProperties.length} canonical · ${observedAddresses.length} observed`}</span>}
          >
            {model.propertyLoading ? (
              <p className="text-sm font-light text-black/45">Loading Property service…</p>
            ) : model.propertyError ? (
              <p className="text-sm font-light text-[var(--portal-archive)]">{model.propertyError}</p>
            ) : canonicalProperties.length === 0 ? (
              <p className="text-sm font-light text-black/45">No canonical Property yet. The Listing fields can create the missing legal-address and physical-property rows.</p>
            ) : (
              <div className="space-y-2">
                {canonicalProperties.map(({ property, relation }) => {
                  const selected = property.id === model.selectedPropertyId
                  return (
                    <button
                      key={`${property.id}:${relation}`}
                      type="button"
                      onClick={() => void controller.dispatch({ operation: "formLens.selectProperty", payload: { propertyId: property.id } })}
                      className={[
                        "w-full rounded-[var(--portal-tab-radius)] border px-3 py-2.5 text-left transition",
                        selected ? "border-[var(--portal-gold)] bg-white/65" : "border-[var(--portal-panel-border)] bg-white/35 hover:bg-white/50",
                      ].join(" ")}
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex items-center gap-2"><Home className="h-3.5 w-3.5 text-[var(--portal-navy-soft)]" aria-hidden /><span className="truncate font-serif text-base font-light text-[var(--portal-navy)]">{property.localName ?? property.displayName}</span></div>
                          <div className="mt-1 pl-5 text-xs font-light text-black/55">{formatPropertyAddress(property.address) || "Address incomplete"}</div>
                          {property.legalOwnerName ? <div className="mt-1 pl-5 text-[10px] font-light text-black/40">Legal owner: {property.legalOwnerName}</div> : null}
                        </div>
                        <span className="shrink-0 rounded-full bg-[var(--portal-blue-pale)] px-2 py-1 text-[9px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">{relation.replaceAll("_", " ")}</span>
                      </div>
                    </button>
                  )
                })}
              </div>
            )}

            {observedAddresses.length > 0 ? (
              <div className="mt-4 border-t border-[var(--portal-panel-border)] pt-3">
                <div className="mb-2 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">Apple address evidence · not truth yet</div>
                {observedAddresses.slice(0, 3).map((observation) => (
                  <div key={observation.sourceKey} className="mt-2 flex gap-2 rounded-[var(--portal-tab-radius)] bg-white/30 px-3 py-2">
                    <MapPin className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--portal-gold-muted)]" aria-hidden />
                    <div className="text-xs font-light text-black/60">{formatPropertyAddress(observation.address) || "Empty observation"}</div>
                  </div>
                ))}
              </div>
            ) : null}
          </Panel>
        </main>

        <section className="portal-glass-panel min-h-0 overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="flex items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-4 py-3">
            <div>
              <div className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">{template.id} · v{template.version}</div>
              <div className="mt-0.5 font-serif text-lg font-light text-[var(--portal-navy)]">{template.displayName}</div>
            </div>
            <div className="flex items-center gap-3">
              <button type="button" disabled={model.manualFields.length === 0} onClick={() => void controller.dispatch({ operation: "formLens.resetDraft", payload: {} })} className="text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">Reset</button>
              <button
                type="button"
                disabled={!promotable || model.savingCanonical || !model.selectedPersonId}
                onClick={() => void controller.dispatch({ operation: "formLens.promoteCanonical", payload: {} })}
                className="inline-flex min-h-8 items-center gap-1.5 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[9px] font-medium uppercase tracking-[0.12em] text-white disabled:opacity-30"
              >
                <Save className="h-3 w-3" aria-hidden />
                {model.savingCanonical ? "Promoting…" : "Promote canonical"}
              </button>
            </div>
          </div>

          <div className="h-full overflow-y-auto px-4 py-4 pb-24">
            <div className="mb-4 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 px-3 py-2 text-[10px] font-light leading-4 text-black/45">
              Canonical Person/Property hydrate first. If a canonical value is missing, the latest LISTING-01 draft may supply it as gold “Listing evidence.” Only the six bound fields write back; contract terms remain local/downstream.
              {model.listingBinding?.formInstanceId ? <div className="mt-1 text-[var(--portal-gold-muted)]">Listing evidence: {model.listingBinding.formInstanceId.slice(0, 8)}… · {evidenceFields.length} fallback field{evidenceFields.length === 1 ? "" : "s"}</div> : null}
              {model.bindingLoading ? <div className="mt-1">Resolving Listing evidence…</div> : null}
              {model.bindingError ? <div className="mt-1 text-[var(--portal-archive)]">{model.bindingError}</div> : null}
              {model.canonicalStatus ? <div className="mt-1 font-medium text-[var(--portal-navy-soft)]">{model.canonicalStatus}</div> : null}
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              {model.fields.map((field) => (
                <div key={field.name} className={field.type === "textarea" ? "sm:col-span-2" : undefined}>
                  <DraftField
                    field={field}
                    onChange={(value) => void controller.dispatch({ operation: "formLens.fieldChanged", payload: { name: field.name, value } })}
                  />
                  {field.name === "legalOwnerName" && !template.fields.some((item) => item.name === "legalOwnerName") ? (
                    <div className="mt-1 text-[9px] font-light text-black/35">Canonical Property qualifier · sidecar-only until the next immutable LISTING template version.</div>
                  ) : null}
                </div>
              ))}
            </div>
          </div>
        </section>
      </div>
    </div>
  )
}
