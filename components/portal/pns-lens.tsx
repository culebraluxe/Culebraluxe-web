"use client"

import { useMemo } from "react"
import {
  AlertTriangle,
  Building2,
  FileText,
  MapPin,
  Network,
  RotateCcw,
  UserRound,
} from "lucide-react"

import { Panel } from "@/components/portal/panel"
import type { TemplateDefinition, TemplateFieldDefinition } from "@/lib/forms/template-types"
import type { PnsFieldBinding, PnsFieldOwner } from "@/lib/forms/pns-field-binding"
import { PnsLensController, type PnsLensOwnerFilter } from "@/ui/pns-lens"
import { usePageController } from "@/ui/runtime"

const inputClass =
  "mt-1 block min-h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-2.5 text-[13px] font-light text-black/70 outline-none focus:border-[var(--portal-navy)]"

const FILTERS: readonly { value: PnsLensOwnerFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "relation", label: "Role / Edge" },
  { value: "property", label: "Property" },
  { value: "contract", label: "Contract" },
  { value: "orphan", label: "Orphans" },
]

function ownerLabel(owner: PnsFieldOwner | null): string {
  switch (owner) {
    case "relation": return "Role / edge"
    case "property": return "Property"
    case "contract": return "Contract"
    default: return "Orphan"
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

function FoundationCard({
  icon,
  title,
  body,
}: {
  icon: React.ReactNode
  title: string
  body: string
}) {
  return (
    <div className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 px-3 py-2.5">
      <div className="flex items-center gap-2 text-[var(--portal-navy)]">
        {icon}
        <span className="font-serif text-base font-light">{title}</span>
      </div>
      <p className="mt-1 text-[11px] font-light leading-4 text-black/50">{body}</p>
    </div>
  )
}

export function PnsLens({ template }: { template: TemplateDefinition }) {
  const controller = useMemo(() => new PnsLensController(template), [template])
  const model = usePageController(controller)

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

  return (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="flex items-start justify-between gap-4 px-1">
        <div>
          <div className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">Architecture Sidecar · Final Boss</div>
          <h1 className="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">P&amp;S Lens · Ownership + Role Map</h1>
          <p className="mt-1 max-w-3xl text-xs font-light text-black/45">
            Active {template.id} v{template.version}. No production Forms, issued documents, Deal state, or database rows are mutated here.
          </p>
        </div>
        <div className="text-right text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
          Person · Firm · Property · Contract<br />
          role is normalized edge data
        </div>
      </div>

      <div className="grid min-h-0 flex-1 gap-4 xl:h-[calc(100dvh-10.5rem)] xl:grid-cols-[260px_minmax(320px,0.9fr)_minmax(430px,1.25fr)]">
        <aside className="min-h-0 space-y-3 overflow-y-auto">
          <Panel compact heading="Five Small Truths">
            <div className="space-y-2">
              <FoundationCard icon={<UserRound className="h-4 w-4" aria-hidden />} title="Person" body="Human identity and intrinsic facts. Buyer, Seller, Broker and Spouse do not become Person columns." />
              <FoundationCard icon={<Building2 className="h-4 w-4" aria-hidden />} title="Firm" body="Durable business/legal identity: bank, brokerage, law/title/appraisal firm, LLC. Contract role still lives on the edge." />
              <FoundationCard icon={<MapPin className="h-4 w-4" aria-hidden />} title="Property" body="Canonical place plus durable registry facts. Relationship says why the place matters." />
              <FoundationCard icon={<FileText className="h-4 w-4" aria-hidden />} title="Contract" body="Participants, contextual roles/capacity, economics, dates, contingencies and immutable legal assertions." />
              <FoundationCard icon={<Network className="h-4 w-4" aria-hidden />} title="Workflow" body="Moves the Contract through time. It consumes Contract truth; it does not own these P&S fields." />
            </div>
          </Panel>

          <Panel compact heading="Role Vocabulary">
            <p className="text-xs font-light leading-5 text-black/55">
              The database already uses <code className="text-[11px]">role</code> for authorization, so the business Role table is <code className="text-[11px]">relation_role</code>. Aliases resolve inside a scope; they never become stored business truth.
            </p>
            <div className="mt-3 space-y-1.5 text-[11px] font-light text-black/50">
              <div><span className="font-medium text-[var(--portal-navy-soft)]">person_person</span> · SPOUSE</div>
              <div><span className="font-medium text-[var(--portal-navy-soft)]">person_firm</span> · BROKER · ATTORNEY · LOAN_OFFICER…</div>
              <div><span className="font-medium text-[var(--portal-navy-soft)]">contract_person</span> · BUYER · SELLER · SELLER_SPOUSE…</div>
              <div><span className="font-medium text-[var(--portal-navy-soft)]">contract_firm</span> · LENDER · ESCROW_HOLDER · BROKERAGE…</div>
            </div>
          </Panel>
        </aside>

        <main className="min-h-0 space-y-4 overflow-y-auto">
          <Panel compact heading="Thread Result">
            <div className="grid grid-cols-2 gap-2">
              <div className="rounded-[var(--portal-tab-radius)] bg-white/35 px-3 py-2">
                <div className="text-2xl font-serif font-light text-[var(--portal-navy)]">{model.audit.templateFieldCount}</div>
                <div className="text-[9px] uppercase tracking-[0.12em] text-black/40">P&amp;S fields</div>
              </div>
              <div className="rounded-[var(--portal-tab-radius)] bg-white/35 px-3 py-2">
                <div className="text-2xl font-serif font-light text-[var(--portal-navy)]">{model.audit.orphanFields.length}</div>
                <div className="text-[9px] uppercase tracking-[0.12em] text-black/40">true orphans</div>
              </div>
              <div className="rounded-[var(--portal-tab-radius)] bg-white/35 px-3 py-2">
                <div className="text-2xl font-serif font-light text-[var(--portal-navy)]">{model.audit.legacyDealFields.length}</div>
                <div className="text-[9px] uppercase tracking-[0.12em] text-black/40">legacy Deal sources</div>
              </div>
              <div className="rounded-[var(--portal-tab-radius)] bg-white/35 px-3 py-2">
                <div className="text-2xl font-serif font-light text-[var(--portal-navy)]">{model.audit.adapterGapFields.length}</div>
                <div className="text-[9px] uppercase tracking-[0.12em] text-black/40">adapter gaps</div>
              </div>
            </div>

            {model.audit.orphanFields.length > 0 ? (
              <div className="mt-3 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-3 py-2 text-xs font-light text-[var(--portal-archive)]">
                <AlertTriangle className="mr-1.5 inline h-3.5 w-3.5" aria-hidden />
                Orphans: {model.audit.orphanFields.join(", ")}
              </div>
            ) : (
              <div className="mt-3 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-3 py-2 text-xs font-light text-black/55">
                All active template fields have a declared owner. The remaining problems are seams, not homeless business facts.
              </div>
            )}
          </Panel>

          <Panel compact heading="What Did Not Disappear">
            <div className="space-y-3 text-xs font-light leading-5 text-black/55">
              <div>
                <div className="font-medium text-[var(--portal-navy-soft)]">Contract persistence</div>
                ContractService exists, but this branch has no canonical SQL Contract table/repository. Migration 117 deliberately does not fake one from Deal or document_form_instance. Contract edges stay at the service-contract seam for this sidecar.
              </div>
              <div>
                <div className="font-medium text-[var(--portal-navy-soft)]">Property adapter · {model.audit.adapterGapFields.length}</div>
                {model.audit.adapterGapFields.join(", ")} are clean Property facts. Migration 117 adds their columns; PropertyService still needs DTO/repository exposure.
              </div>
              <div>
                <div className="font-medium text-[var(--portal-navy-soft)]">Projection pressure · {model.audit.projectionPressureFields.length}</div>
                Singular Buyer/Seller/Broker fields, legal Person-vs-Firm parties, resolved service providers, and the current Financing “Show All” value need projection decisions. None requires a new domain noun.
              </div>
              <div>
                <div className="font-medium text-[var(--portal-navy-soft)]">Legacy Deal source · {model.audit.legacyDealFields.length}</div>
                {model.audit.legacyDealFields.join(", ")} still arrive from deal.* in the XML today. The sidecar assigns them new ownership without expanding Deal.
              </div>
            </div>
          </Panel>

          <Panel compact heading="3NF Edge Shape">
            <pre className="overflow-x-auto whitespace-pre-wrap text-[11px] font-light leading-5 text-black/55">{`Person ── role ── Person
Person ── role ── Firm
Person ── context ── Property
Firm   ── role ── Property
Contract ── role ── Person
Contract ── role ── Firm
Contract ── SUBJECT_PROPERTY ── Property

role aliases → canonical scoped role code
owning service → validates edge
mapping table → persists edge`}</pre>
          </Panel>
        </main>

        <section className="portal-glass-panel min-h-0 overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="border-b border-[var(--portal-panel-border)] px-4 py-3">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">{template.id} · v{template.version}</div>
                <div className="mt-0.5 font-serif text-lg font-light text-[var(--portal-navy)]">Field Ownership Audit</div>
              </div>
              <button
                type="button"
                onClick={() => void controller.dispatch({ operation: "pnsLens.reset", payload: {} })}
                className="inline-flex items-center gap-1.5 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)]"
              >
                <RotateCcw className="h-3 w-3" aria-hidden /> Reset
              </button>
            </div>

            <div className="mt-3 flex flex-wrap gap-1.5">
              {FILTERS.map((filter) => {
                const selected = filter.value === model.ownerFilter
                const count = filter.value === "all"
                  ? model.fields.length
                  : filter.value === "orphan"
                    ? ownerCounts.orphan
                    : ownerCounts[filter.value]
                return (
                  <button
                    key={filter.value}
                    type="button"
                    onClick={() => void controller.dispatch({ operation: "pnsLens.ownerChanged", payload: { owner: filter.value } })}
                    className={[
                      "rounded-full border px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.1em] transition",
                      selected
                        ? "border-[var(--portal-gold)] bg-white/70 text-[var(--portal-navy)]"
                        : "border-[var(--portal-panel-border)] bg-white/30 text-black/45 hover:bg-white/50",
                    ].join(" ")}
                  >
                    {filter.label} · {count}
                  </button>
                )
              })}
            </div>
          </div>

          <div className="h-full overflow-y-auto px-4 py-4 pb-24">
            <div className="space-y-4">
              {visibleFields.map(({ definition, binding, value }) => {
                const readiness = readinessLabel(binding)
                return (
                  <div key={definition.name} className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-3">
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <div className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">{definition.name}</div>
                        <div className="mt-0.5 text-[13px] font-medium text-[var(--portal-navy)]">{definition.label}{definition.required ? " *" : ""}</div>
                      </div>
                      <div className="shrink-0 text-right">
                        <div className="text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-gold-muted)]">{ownerLabel(binding?.owner ?? null)}</div>
                        {readiness ? <div className="mt-0.5 text-[8px] uppercase tracking-[0.08em] text-[var(--portal-archive)]">{readiness}</div> : null}
                      </div>
                    </div>

                    <FieldInput
                      field={definition}
                      value={value}
                      onChange={(next) => void controller.dispatch({ operation: "pnsLens.fieldChanged", payload: { field: definition.name, value: next } })}
                    />

                    <div className="mt-2 border-t border-[var(--portal-panel-border)] pt-2 text-[10px] font-light leading-4 text-black/45">
                      {binding ? (
                        <>
                          <div><span className="font-medium text-[var(--portal-navy-soft)]">→</span> {binding.path}</div>
                          {binding.relations?.map((edge) => (
                            <div key={`${edge.scope}:${edge.roleCode}:${edge.target}`} className="mt-0.5">
                              <span className="font-medium text-[var(--portal-navy-soft)]">role</span> {edge.scope}:{edge.roleCode} → {edge.target}
                            </div>
                          ))}
                          {definition.binding?.startsWith("deal.") ? (
                            <div className="mt-0.5 text-[var(--portal-archive)]">legacy source: {definition.binding}</div>
                          ) : null}
                          {definition.when ? (
                            <div className="mt-0.5">visible when {definition.when.field} = {definition.when.values.join(" / ")}</div>
                          ) : null}
                          {binding.note ? <div className="mt-1 text-black/55">{binding.note}</div> : null}
                        </>
                      ) : (
                        <div className="text-[var(--portal-archive)]">No ownership binding. This is a real orphan.</div>
                      )}
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
