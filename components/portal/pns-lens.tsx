"use client"

import { useEffect, useMemo } from "react"
import {
  AlertTriangle,
  Building2,
  FileText,
  MapPin,
  Network,
  RotateCcw,
  Save,
  UserRound,
} from "lucide-react"

import { Panel } from "@/components/portal/panel"
import type { PnsFieldOrigin } from "@/lib/forms/pns-canonical-types"
import type { TemplateDefinition, TemplateFieldDefinition } from "@/lib/forms/template-types"
import type { PnsFieldBinding, PnsFieldOwner } from "@/lib/forms/pns-field-binding"
import {
  HttpPnsLensSource,
  PnsLensController,
  type PnsLensOwnerFilter,
} from "@/ui/pns-lens"
import { usePageController } from "@/ui/runtime"

const inputClass =
  "mt-1 block min-h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-2.5 text-[13px] font-light text-black/70 outline-none focus:border-[var(--portal-navy)]"

const FILTERS: readonly { value: PnsLensOwnerFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "relation", label: "Role" },
  { value: "property", label: "Property" },
  { value: "contract", label: "Contract" },
  { value: "orphan", label: "Orphans" },
]

function ownerLabel(owner: PnsFieldOwner | null): string {
  switch (owner) {
    case "relation": return "Role"
    case "property": return "Property"
    case "contract": return "Contract"
    default: return "Orphan"
  }
}

function originLabel(origin: PnsFieldOrigin): string {
  switch (origin) {
    case "contract": return "Contract"
    case "role": return "Role"
    case "person": return "Person"
    case "firm": return "Firm"
    case "property": return "Property"
    case "pns_form": return "P&S evidence"
    case "template_default": return "Default"
    case "manual": return "Edited"
    default: return "Needs input"
  }
}

function readinessLabel(binding: PnsFieldBinding | null): string | null {
  if (!binding || binding.readiness === "clean") return null
  if (binding.readiness === "adapter_gap") return "Adapter gap"
  return "Projection pressure"
}

function FieldInput({
  field,
  value,
  onChange,
}: {
  field: TemplateFieldDefinition
  value: string
  onChange: (value: string) => void
}) {
  if (field.type === "select") {
    return (
      <select value={value} onChange={(event) => onChange(event.target.value)} className={inputClass}>
        <option value="">Select…</option>
        {(field.options ?? []).map((option) => <option key={option} value={option}>{option}</option>)}
      </select>
    )
  }

  if (field.type === "textarea") {
    return <textarea value={value} onChange={(event) => onChange(event.target.value)} className={`${inputClass} min-h-20 py-2`} />
  }

  return (
    <input
      type={field.type === "date" ? "date" : "text"}
      inputMode={field.type === "money" ? "decimal" : undefined}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      className={inputClass}
    />
  )
}

function compactAddress(property: { address: { addressLine1: string | null; city: string | null } }): string {
  return [property.address.addressLine1, property.address.city].filter(Boolean).join(", ")
}

export function PnsLens({ template }: { template: TemplateDefinition }) {
  const source = useMemo(() => new HttpPnsLensSource(), [])
  const controller = useMemo(() => new PnsLensController(source, template), [source, template])
  const model = usePageController(controller)

  useEffect(() => {
    void controller.dispatch({ operation: "pnsLens.load", payload: {} })
    // Keep memoized controller alive through React StrictMode cleanup/setup.
  }, [controller])

  const visibleFields = model.fields.filter((field) => {
    if (model.ownerFilter === "all") return true
    if (model.ownerFilter === "orphan") return field.binding === null
    return field.binding?.owner === model.ownerFilter
  })

  const ownerCounts = model.fields.reduce<Record<PnsFieldOwner | "orphan", number>>(
    (counts, field) => {
      const owner = field.binding?.owner ?? "orphan"
      counts[owner] += 1
      return counts
    },
    { relation: 0, property: 0, contract: 0, orphan: 0 },
  )
  const properties = model.propertyContext?.properties ?? []

  return (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="flex items-start justify-between gap-4 px-1">
        <div>
          <div className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">Architecture Sidecar · Canonical Round Trip</div>
          <h1 className="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">P&amp;S Lens · Form → Contract</h1>
          <p className="mt-1 max-w-3xl text-xs font-light text-black/45">
            Active {template.id} v{template.version}. Save writes reviewed Person / Firm / Property / Contract draft truth only; Deal and issued documents remain untouched.
          </p>
        </div>
        <div className="text-right text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
          Person · Firm · Property → Form → Contract<br />
          Workflow consumes Contract next
        </div>
      </div>

      <div className="grid min-h-0 flex-1 gap-4 xl:h-[calc(100dvh-10.5rem)] xl:grid-cols-[260px_minmax(320px,0.9fr)_minmax(430px,1.25fr)]">
        <aside className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="shrink-0 border-b border-[var(--portal-panel-border)] p-3">
            <div className="mb-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">Seller / Person · {model.total.toLocaleString()}</div>
            <input
              type="search"
              value={model.query}
              onChange={(event) => void controller.dispatch({ operation: "pnsLens.queryChanged", payload: { query: event.target.value } })}
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
                  onClick={() => void controller.dispatch({ operation: "pnsLens.selectPerson", payload: { personId: item.id } })}
                  className={[
                    "w-full border-b border-[var(--portal-panel-border)] px-3 py-2.5 text-left transition",
                    selected ? "border-l-2 border-l-[var(--portal-gold)] bg-white/45" : "border-l-2 border-l-transparent hover:bg-white/25",
                  ].join(" ")}
                >
                  <div className="truncate text-[13px] font-medium text-[var(--portal-navy)]">{item.nameResolved ? item.displayName : "Unknown contact"}</div>
                  <div className="truncate text-[11px] font-light text-black/45">{item.primaryPhone ?? item.primaryEmail ?? item.role}</div>
                </button>
              )
            })}
          </div>
          <div className="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-2">
            <button type="button" disabled={model.page <= 1} onClick={() => void controller.dispatch({ operation: "pnsLens.previousPage", payload: {} })} className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">← Prev</button>
            <span className="text-[10px] font-light text-black/40">{model.page} / {model.pageCount}</span>
            <button type="button" disabled={model.page >= model.pageCount} onClick={() => void controller.dispatch({ operation: "pnsLens.nextPage", payload: {} })} className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">Next →</button>
          </div>
        </aside>

        <main className="min-h-0 space-y-4 overflow-y-auto">
          <Panel compact heading="Canonical Context">
            {model.contextLoading ? (
              <p className="text-sm font-light text-black/45">Loading Person + Property…</p>
            ) : model.contextError ? (
              <p className="text-sm font-light text-[var(--portal-archive)]">{model.contextError}</p>
            ) : model.client ? (
              <div>
                <div className="flex items-center gap-2">
                  <UserRound className="h-4 w-4 text-[var(--portal-navy-soft)]" aria-hidden />
                  <span className="font-serif text-lg font-light text-[var(--portal-navy)]">{model.client.displayName}</span>
                </div>
                <div className="mt-1 text-xs font-light text-black/45">{model.client.email ?? "No email"} · {model.client.phone ?? "No phone"}</div>
              </div>
            ) : <p className="text-sm font-light text-black/45">Select a Person.</p>}

            <div className="mt-3 border-t border-[var(--portal-panel-border)] pt-3">
              <div className="mb-2 flex items-center gap-2 text-[10px] font-medium uppercase tracking-[0.14em] text-black/40"><MapPin className="h-3.5 w-3.5" aria-hidden /> Subject Property</div>
              {properties.length === 0 ? (
                <p className="text-xs font-light text-black/45">No canonical Property yet. P&amp;S property facts can create the physical Property on save.</p>
              ) : (
                <div className="space-y-1.5">
                  {properties.map(({ property, relation }) => {
                    const selected = property.id === model.selectedPropertyId
                    return (
                      <button
                        key={`${property.id}:${relation}`}
                        type="button"
                        onClick={() => void controller.dispatch({ operation: "pnsLens.selectProperty", payload: { propertyId: property.id } })}
                        className={[
                          "w-full rounded-[var(--portal-tab-radius)] border px-3 py-2 text-left transition",
                          selected ? "border-[var(--portal-gold)] bg-white/65" : "border-[var(--portal-panel-border)] bg-white/30 hover:bg-white/45",
                        ].join(" ")}
                      >
                        <div className="text-sm font-medium text-[var(--portal-navy)]">{property.localName ?? property.displayName}</div>
                        <div className="mt-0.5 text-[10px] font-light text-black/45">{relation.replaceAll("_", " ")} · {compactAddress(property) || "address incomplete"}</div>
                      </button>
                    )
                  })}
                </div>
              )}
            </div>
          </Panel>

          <Panel compact heading="Contract Draft">
            {model.bindingLoading ? (
              <p className="text-sm font-light text-black/45">Hydrating P&amp;S truth…</p>
            ) : model.bindingError ? (
              <p className="text-sm font-light text-[var(--portal-archive)]">{model.bindingError}</p>
            ) : (
              <div className="space-y-2 text-xs font-light text-black/55">
                <div className="flex items-center gap-2"><FileText className="h-4 w-4 text-[var(--portal-navy-soft)]" aria-hidden /><span className="font-medium text-[var(--portal-navy)]">{model.canonical?.contractId ?? "New Contract draft"}</span></div>
                <div>Status: {model.canonical?.contractStatus ?? "not persisted"}</div>
                <div>Role assignments: {model.canonical?.roles.length ?? 0}</div>
                <div>Source P&amp;S evidence: {model.canonical?.formInstanceId ?? "none"}</div>
              </div>
            )}
            {model.saveStatus ? <div className="mt-3 text-xs font-medium text-[var(--portal-gold-muted)]">{model.saveStatus}</div> : null}
          </Panel>

          <Panel compact heading="Thread Result">
            <div className="grid grid-cols-2 gap-2">
              <div className="rounded-[var(--portal-tab-radius)] bg-white/35 px-3 py-2"><div className="text-2xl font-serif font-light text-[var(--portal-navy)]">{model.audit.templateFieldCount}</div><div className="text-[9px] uppercase tracking-[0.12em] text-black/40">P&amp;S fields</div></div>
              <div className="rounded-[var(--portal-tab-radius)] bg-white/35 px-3 py-2"><div className="text-2xl font-serif font-light text-[var(--portal-navy)]">{model.audit.orphanFields.length}</div><div className="text-[9px] uppercase tracking-[0.12em] text-black/40">true orphans</div></div>
            </div>
            <pre className="mt-3 overflow-x-auto whitespace-pre-wrap text-[11px] font-light leading-5 text-black/55">{`Person / Firm / Property
          ↓
       P&S Form
          ↓
Contract facts + Role mappings
          ↓
       Workflow`}</pre>
          </Panel>

          <Panel compact heading="Role Vocabulary">
            <div className="flex gap-2"><Building2 className="mt-0.5 h-4 w-4 shrink-0 text-[var(--portal-navy-soft)]" aria-hidden /><p className="text-xs font-light leading-5 text-black/55"><code>security_role</code> owns application authorization. <code>role</code> owns business positions such as SELLER, BUYER, LENDER and CLOSING_NOTARY. Contract mappings bind Person/Firm IDs to those positions.</p></div>
          </Panel>

          <Panel compact heading="Next Seam">
            <div className="flex gap-2"><Network className="mt-0.5 h-4 w-4 shrink-0 text-[var(--portal-navy-soft)]" aria-hidden /><p className="text-xs font-light leading-5 text-black/55">Once this draft path is proven against DEV, Workflow can read Contract facts directly and Deal can stop owning duplicate P&amp;S truth.</p></div>
          </Panel>
        </main>

        <section className="portal-glass-panel min-h-0 overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="border-b border-[var(--portal-panel-border)] px-4 py-3">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">{template.id} · v{template.version}</div>
                <div className="mt-0.5 font-serif text-lg font-light text-[var(--portal-navy)]">Canonical P&amp;S Draft</div>
              </div>
              <div className="flex items-center gap-3">
                <button type="button" onClick={() => void controller.dispatch({ operation: "pnsLens.reset", payload: {} })} className="inline-flex items-center gap-1.5 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)]"><RotateCcw className="h-3 w-3" aria-hidden /> Reset</button>
                <button
                  type="button"
                  disabled={!model.selectedPersonId || model.bindingLoading || model.saving}
                  onClick={() => void controller.dispatch({ operation: "pnsLens.save", payload: {} })}
                  className="inline-flex items-center gap-1.5 rounded-full border border-[var(--portal-gold)] bg-white/60 px-3 py-1.5 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)] disabled:opacity-35"
                >
                  <Save className="h-3 w-3" aria-hidden /> {model.saving ? "Saving…" : "Save Draft"}
                </button>
              </div>
            </div>

            <div className="mt-3 flex flex-wrap gap-1.5">
              {FILTERS.map((filter) => {
                const selected = filter.value === model.ownerFilter
                const count = filter.value === "all" ? model.fields.length : filter.value === "orphan" ? ownerCounts.orphan : ownerCounts[filter.value]
                return (
                  <button key={filter.value} type="button" onClick={() => void controller.dispatch({ operation: "pnsLens.ownerChanged", payload: { owner: filter.value } })} className={["rounded-full border px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.1em] transition", selected ? "border-[var(--portal-gold)] bg-white/70 text-[var(--portal-navy)]" : "border-[var(--portal-panel-border)] bg-white/30 text-black/45 hover:bg-white/50"].join(" ")}>{filter.label} · {count}</button>
                )
              })}
            </div>
          </div>

          <div className="h-full overflow-y-auto px-4 py-4 pb-24">
            {model.audit.orphanFields.length > 0 ? (
              <div className="mb-3 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-3 py-2 text-xs font-light text-[var(--portal-archive)]"><AlertTriangle className="mr-1.5 inline h-3.5 w-3.5" aria-hidden />Orphans: {model.audit.orphanFields.join(", ")}</div>
            ) : null}
            <div className="space-y-4">
              {visibleFields.map(({ definition, binding, value, origin }) => {
                const readiness = readinessLabel(binding)
                return (
                  <div key={definition.name} className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-3">
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0"><div className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">{definition.name}</div><div className="mt-0.5 text-[13px] font-medium text-[var(--portal-navy)]">{definition.label}{definition.required ? " *" : ""}</div></div>
                      <div className="shrink-0 text-right"><div className="text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-gold-muted)]">{ownerLabel(binding?.owner ?? null)}</div><div className="mt-0.5 text-[8px] uppercase tracking-[0.08em] text-black/40">{originLabel(origin)}</div>{readiness ? <div className="mt-0.5 text-[8px] uppercase tracking-[0.08em] text-[var(--portal-archive)]">{readiness}</div> : null}</div>
                    </div>
                    <FieldInput field={definition} value={value} onChange={(next) => void controller.dispatch({ operation: "pnsLens.fieldChanged", payload: { field: definition.name, value: next } })} />
                    <div className="mt-2 border-t border-[var(--portal-panel-border)] pt-2 text-[10px] font-light leading-4 text-black/45">
                      {binding ? <><div><span className="font-medium text-[var(--portal-navy-soft)]">→</span> {binding.path}</div>{binding.relations?.map((edge) => <div key={`${edge.scope}:${edge.roleCode}:${edge.target}`} className="mt-0.5"><span className="font-medium text-[var(--portal-navy-soft)]">role</span> {edge.scope}:{edge.roleCode} → {edge.target}</div>)}{definition.binding?.startsWith("deal.") ? <div className="mt-0.5 text-[var(--portal-archive)]">legacy input only: {definition.binding}</div> : null}{binding.note ? <div className="mt-1 text-black/55">{binding.note}</div> : null}</> : <div className="text-[var(--portal-archive)]">No ownership binding. This is a real orphan.</div>}
                    </div>
                  </div>
                )
              })}
            </div>
          </div>
        </section>
      </div>
    </div>
  )
}
