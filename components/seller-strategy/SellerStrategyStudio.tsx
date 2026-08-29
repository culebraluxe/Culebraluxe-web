"use client"

import { useMemo, useState } from "react"

import { DEFAULT_INPUTS } from "@/lib/decision-analysis/defaults"
import { evaluate, money, pct } from "@/lib/decision-analysis/model"
import { buildDecisionPdf, downloadPdf } from "@/lib/decision-analysis/pdf"
import type { Inputs, ModelResult, OptionId } from "@/lib/decision-analysis/types"
import {
  buildRecommendationRationale,
  buildTakeaways,
  rankStrategies,
} from "@/lib/seller-strategy/insights"
import { TreeSvg } from "@/components/decision-analysis/TreeSvg"

type NumKind = "money" | "months" | "pct"

type Field = { key: keyof Inputs; label: string; kind: "text" | NumKind }

const PARENT_KEY: Field[] = [
  { key: "propertyName", label: "Name", kind: "text" },
  { key: "appraisal", label: "Appraisal", kind: "money" },
  { key: "sellingCostPct", label: "Selling costs %", kind: "pct" },
  { key: "discountRate", label: "Discount %", kind: "pct" },
  { key: "taxRate", label: "Tax placeholder %", kind: "pct" },
]

const PARENT_BASIS: Field[] = [
  { key: "purchasePrice", label: "Purchase / basis", kind: "money" },
  { key: "extraSpent", label: "Extra spent", kind: "money" },
  { key: "contributoryToDate", label: "Contributory extra", kind: "money" },
]

const STRATEGIES: {
  id: OptionId
  onKey: keyof Inputs
  title: string
  short: string
  blurb: string
  accent: string
  fields: Field[]
}[] = [
  {
    id: 1, onKey: "o1On", title: "Sell As-Is", short: "As-is",
    blurb: "Same house. Extra kit is salvage.", accent: "#1B365D",
    fields: [
      { key: "o1Months", label: "Months", kind: "months" },
      { key: "o1Salvage", label: "Salvage", kind: "money" },
      { key: "o1LowDelta", label: "Low vs appraisal %", kind: "pct" },
      { key: "o1MidDelta", label: "Mid vs appraisal %", kind: "pct" },
      { key: "o1HighDelta", label: "Ideal vs appraisal %", kind: "pct" },
      { key: "o1PLow", label: "P low %", kind: "pct" },
      { key: "o1PMid", label: "P mid %", kind: "pct" },
      { key: "o1PHigh", label: "P ideal %", kind: "pct" },
    ],
  },
  {
    id: 2, onKey: "o2On", title: "Improve then Sell", short: "Improve",
    blurb: "Same product, more brick.", accent: "#0F6E6B",
    fields: [
      { key: "o2Capex", label: "Capex", kind: "money" },
      { key: "o2Months", label: "Months", kind: "months" },
      { key: "o2Recovery", label: "Recovery %", kind: "pct" },
      { key: "o2Salvage", label: "Salvage", kind: "money" },
      { key: "o2LowDelta", label: "Low vs improved %", kind: "pct" },
      { key: "o2MidDelta", label: "Base vs improved %", kind: "pct" },
      { key: "o2HighDelta", label: "High vs improved %", kind: "pct" },
      { key: "o2PLow", label: "P low %", kind: "pct" },
      { key: "o2PMid", label: "P base %", kind: "pct" },
      { key: "o2PHigh", label: "P high %", kind: "pct" },
    ],
  },
  {
    id: 3, onKey: "o3On", title: "Join", short: "Join",
    blurb: "One ticket: house + business.", accent: "#B85C38",
    fields: [
      { key: "o3Capex", label: "Launch cash", kind: "money" },
      { key: "o3Months", label: "Months", kind: "months" },
      { key: "o3Noi", label: "Stabilized NOI", kind: "money" },
      { key: "o3CapRate", label: "Cap rate %", kind: "pct" },
      { key: "o3PSuccess", label: "P stabilize %", kind: "pct" },
      { key: "o3FailSalvage", label: "Fail salvage price", kind: "money" },
      { key: "o3LowDelta", label: "Low vs NOI/cap %", kind: "pct" },
      { key: "o3MidDelta", label: "Base vs NOI/cap %", kind: "pct" },
      { key: "o3HighDelta", label: "High vs NOI/cap %", kind: "pct" },
      { key: "o3PLow", label: "P low | success %", kind: "pct" },
      { key: "o3PMid", label: "P base | success %", kind: "pct" },
      { key: "o3PHigh", label: "P high | success %", kind: "pct" },
    ],
  },
  {
    id: 4, onKey: "o4On", title: "Fork", short: "Fork",
    blurb: "House and assets separate.", accent: "#6C3483",
    fields: [
      { key: "o4Capex", label: "Split cost", kind: "money" },
      { key: "o4Months", label: "Months", kind: "months" },
      { key: "o4AssetBase", label: "Asset proceeds base", kind: "money" },
      { key: "o4Salvage", label: "Leftover salvage", kind: "money" },
      { key: "o4LowDelta", label: "Low %", kind: "pct" },
      { key: "o4MidDelta", label: "Mid %", kind: "pct" },
      { key: "o4HighDelta", label: "High %", kind: "pct" },
      { key: "o4PLow", label: "P low %", kind: "pct" },
      { key: "o4PMid", label: "P mid %", kind: "pct" },
      { key: "o4PHigh", label: "P high %", kind: "pct" },
    ],
  },
  {
    id: 5, onKey: "o5On", title: "Hold", short: "Hold",
    blurb: "Keep or sell only a share.", accent: "#7D6608",
    fields: [
      { key: "o5Share", label: "Share sold %", kind: "pct" },
      { key: "o5Months", label: "Months", kind: "months" },
      { key: "o5PeriodCash", label: "Income/(carry) over period", kind: "money" },
      { key: "o5Capex", label: "Keep-up capex", kind: "money" },
      { key: "o5LowDelta", label: "Low vs appraisal %", kind: "pct" },
      { key: "o5MidDelta", label: "Mid vs appraisal %", kind: "pct" },
      { key: "o5HighDelta", label: "High vs appraisal %", kind: "pct" },
      { key: "o5PLow", label: "P low %", kind: "pct" },
      { key: "o5PMid", label: "P mid %", kind: "pct" },
      { key: "o5PHigh", label: "P high %", kind: "pct" },
    ],
  },
]

function readValue(inputs: Inputs, kind: Field["kind"], key: keyof Inputs): string {
  const v = inputs[key]
  if (kind === "text") return String(v)
  if (typeof v === "boolean") return v ? "true" : "false"
  if (kind === "pct") return String(Math.round(Number(v) * 1000) / 10)
  return String(v)
}

function writeValue(kind: Field["kind"], raw: string): string | number {
  if (kind === "text") return raw
  const n = Number(raw)
  if (Number.isNaN(n)) return 0
  if (kind === "pct") return n / 100
  return n
}

function Field({
  inputs,
  setInputs,
  field,
  disabled,
}: {
  inputs: Inputs
  setInputs: React.Dispatch<React.SetStateAction<Inputs>>
  field: Field
  disabled?: boolean
}) {
  const key = field.key
  return (
    <label className="block">
      <span className="mb-0.5 block text-[10px] uppercase tracking-wide text-[var(--portal-muted)]">
        {field.label}
      </span>
      <input
        disabled={disabled}
        className="w-full rounded-md border border-[var(--portal-border)] bg-white/60 px-2 py-1.5 text-sm text-[var(--portal-text)] outline-none transition focus:border-[var(--portal-gold)] disabled:text-[var(--portal-muted)]"
        value={readValue(inputs, field.kind, key)}
        onChange={(e) =>
          setInputs((prev) => ({ ...prev, [key]: writeValue(field.kind, e.target.value) }))
        }
      />
    </label>
  )
}

export function SellerStrategyStudio() {
  const [inputs, setInputs] = useState<Inputs>(DEFAULT_INPUTS)
  const [busy, setBusy] = useState(false)
  const [editAll, setEditAll] = useState(false)
  const [activeEdit, setActiveEdit] = useState<OptionId | null>(null)
  const [showDetail, setShowDetail] = useState(false)

  const model: ModelResult = useMemo(() => evaluate(inputs), [inputs])
  const ranking = useMemo(() => rankStrategies(model), [model])
  const rationale = useMemo(() => buildRecommendationRationale(model, inputs), [model, inputs])
  const takeaways = useMemo(() => buildTakeaways(model, inputs), [model, inputs])

  async function onPdf() {
    setBusy(true)
    try {
      const bytes = await buildDecisionPdf(inputs, model)
      const slug = inputs.propertyName.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")
      downloadPdf(bytes, `${slug || "seller-strategy"}.pdf`)
    } finally {
      setBusy(false)
    }
  }

  const activeStrategy = STRATEGIES.find((s) => s.id === activeEdit) ?? null
  const maxPv = ranking.length ? ranking[0].emvPv : 1
  const activeScore = activeStrategy ? model.scores.find((s) => s.option === activeStrategy.id) : undefined


  return (
    <div className="space-y-5">
      {/* Page header */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <p className="text-xs uppercase tracking-[0.22em] text-[var(--portal-gold-muted)]">
            Core · Strategic disposition
          </p>
          <h1 className="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">
            Seller Strategy
          </h1>
          <p className="mt-1 text-sm text-[var(--portal-muted)]">
            Strategic disposition analysis for sellers — change any assumption and the model recalculates live.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setInputs(DEFAULT_INPUTS)}
            className="rounded-full border border-[var(--portal-border)] bg-white/50 px-4 py-1.5 text-xs font-medium text-[var(--portal-navy)] transition hover:bg-white/70"
          >
            Reset
          </button>
          <button
            type="button"
            onClick={onPdf}
            disabled={busy}
            className="rounded-full bg-[var(--portal-gold)] px-4 py-1.5 text-xs font-semibold text-[var(--portal-navy)] shadow-sm transition hover:bg-[var(--portal-gold-soft)] disabled:opacity-60"
          >
            {busy ? "Building PDF…" : "Download PDF"}
          </button>
        </div>
      </div>

      {/* Model validity flags */}
      {model.flags.length > 0 && (
        <div className="rounded-2xl border border-[var(--portal-danger)]/40 bg-[var(--portal-danger)]/10 px-4 py-3 text-sm text-[var(--portal-danger)]">
          {model.flags.map((f) => (
            <p key={f}>{f}</p>
          ))}
        </div>
      )}

      {/* A/B. Permanent Executive Summary shell: both regions stay inside one glass surface. */}
      <section className="portal-glass-panel portal-glass-panel-lifted overflow-hidden rounded-2xl p-1">
        <div className="grid grid-cols-[minmax(0,2fr)_minmax(280px,1fr)] gap-1">
          {/* A. Recommendation hero */}
          <div className="portal-glass-panel portal-glass-panel-feature min-w-0 rounded-xl p-5">
            <div className="grid grid-cols-[minmax(0,1fr)_minmax(240px,300px)] gap-6">
              <div>
                <p className="text-xs uppercase tracking-[0.24em] text-[var(--portal-feature-eyebrow)]">Recommended</p>
                <h2 className="mt-2 font-serif text-3xl font-light text-white">{model.winner.name}</h2>
                <p className="mt-2 text-lg text-white">
                  {money(model.winner.emvPv)}{" "}
                  <span className="text-sm text-[var(--portal-feature-muted)]">expected PV</span>
                  <span className="mx-2 text-[var(--portal-feature-muted)]">·</span>
                  {model.winner.months}
                  <span className="text-sm text-[var(--portal-feature-muted)]"> months to liquidity</span>
                </p>
                <div className="mt-3 rounded-xl border border-white/12 bg-white/5 p-3">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-feature-eyebrow)]">
                    Why this strategy?
                  </p>
                  <p className="mt-1 text-sm leading-5 text-[var(--portal-feature-muted)]">{rationale}</p>
                </div>
              </div>

              <div className="rounded-xl border border-white/12 bg-white/5 p-4">
                <p className="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-feature-eyebrow)]">
                  Expected PV Ranking
                </p>
                <div className="mt-3 space-y-2.5">
                  {ranking.map((s) => {
                    const width = maxPv > 0 ? Math.max(4, (s.emvPv / maxPv) * 100) : 0
                    return (
                      <div key={s.option} className="flex items-center gap-2">
                        <span className="w-14 flex-none text-[11px] font-medium text-white">{s.short}</span>
                        <div className="h-2 flex-1 overflow-hidden rounded-full bg-white/12">
                          <div
                            className="h-full rounded-full"
                            style={{ width: `${width}%`, backgroundColor: s.best ? "var(--portal-gold)" : "rgba(255,255,255,0.5)" }}
                          />
                        </div>
                        <span className="w-20 flex-none text-right text-[11px] text-[var(--portal-feature-muted)]">
                          {money(s.emvPv)}
                        </span>
                      </div>
                    )
                  })}
                </div>
              </div>
            </div>
          </div>

          {/* B. Live Assumptions */}
          <div className="portal-glass-panel portal-glass-panel-soft min-w-0 rounded-xl p-5">
            <div className="mb-3 flex items-end justify-between gap-3">
              <div>
                <p className="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">Shared facts</p>
                <h2 className="font-serif text-lg font-light text-[var(--portal-navy)]">Live Assumptions</h2>
              </div>
              <p className="text-xs text-[var(--portal-muted)]">
                Sunk {money(inputs.purchasePrice + inputs.extraSpent)} · basis {money(model.basis)}
              </p>
            </div>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              {PARENT_KEY.map((f) =>
                f.key === "propertyName" ? (
                  <div key={String(f.key)} className="sm:col-span-2">
                    <Field inputs={inputs} setInputs={setInputs} field={f} />
                  </div>
                ) : (
                  <Field key={String(f.key)} inputs={inputs} setInputs={setInputs} field={f} />
                ),
              )}
            </div>
            <button
              type="button"
              onClick={() => setEditAll((v) => !v)}
              className="mt-3 text-xs font-medium text-[var(--portal-navy)] underline decoration-[var(--portal-gold)] underline-offset-2 hover:text-[var(--portal-navy-soft)]"
            >
              {editAll ? "Hide" : "Edit"} basis & sunk-cost fields
            </button>
            {editAll && (
              <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2">
                {PARENT_BASIS.map((f) => (
                  <Field key={String(f.key)} inputs={inputs} setInputs={setInputs} field={f} />
                ))}
              </div>
            )}
          </div>
        </div>
      </section>

      {/* C. Five strategy cards */}
      <section className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-5">
        {STRATEGIES.map((opt) => {
          const score = model.scores.find((s) => s.option === opt.id)
          const on = Boolean(inputs[opt.onKey])
          return (
            <article
              key={opt.id}
              className={`portal-glass-panel portal-glass-panel-soft portal-glass-panel-lifted rounded-2xl p-4 transition ${on ? "" : "opacity-55"}`}
              style={{ borderTopColor: opt.accent, borderTopWidth: 2 }}
            >
              <div className="mb-2 flex items-start justify-between gap-2">
                <div>
                  <h3 className="text-sm font-semibold text-[var(--portal-navy)]">{opt.title}</h3>
                  <p className="text-[11px] text-[var(--portal-muted)]">{opt.blurb}</p>
                </div>
                <label className="flex items-center gap-1 text-[10px] uppercase tracking-wide text-[var(--portal-muted)]">
                  <input
                    type="checkbox"
                    checked={on}
                    onChange={(e) => setInputs((prev) => ({ ...prev, [opt.onKey]: e.target.checked }))}
                  />
                  On
                </label>
              </div>
              <p className={`text-xl font-semibold ${score?.best && on ? "text-[var(--portal-gold-muted)]" : "text-[var(--portal-navy)]"}`}>
                {on ? money(score?.emvPv ?? 0) : "—"}
              </p>
              <p className="mt-1 text-[11px] text-[var(--portal-muted)]">
                {score?.months ?? 0} mo · future {money(score?.futureCash ?? 0)}
                {score?.best && on ? " · BEST" : ""}
              </p>
              <button
                type="button"
                onClick={() => setActiveEdit((prev) => (prev === opt.id ? null : opt.id))}
                disabled={!on}
                className="mt-3 w-full rounded-full border border-[var(--portal-border)] bg-white/50 px-3 py-1.5 text-[11px] font-medium text-[var(--portal-navy)] transition hover:bg-white/70 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {activeEdit === opt.id ? "Close Assumptions" : "Edit Assumptions"}
              </button>
            </article>
          )
        })}
      </section>

      {/* C2. Inline Edit Assumptions — expands in the page, no modal/drawer */}
      {activeStrategy && (
        <section
          className="portal-glass-panel portal-glass-panel-soft portal-glass-panel-lifted rounded-2xl p-5"
          style={{ borderTopColor: activeStrategy.accent, borderTopWidth: 2 }}
        >
          <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
            <div>
              <p className="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">
                {activeStrategy.short.toUpperCase()} ASSUMPTIONS
              </p>
              <h3 className="mt-1 font-serif text-lg font-light text-[var(--portal-navy)]">
                {activeStrategy.title}
              </h3>
              <p className="text-xs text-[var(--portal-muted)]">
                {money(activeScore?.emvPv ?? 0)} expected PV · changes apply immediately
              </p>
            </div>
            <button
              type="button"
              onClick={() => setActiveEdit(null)}
              className="rounded-full border border-[var(--portal-border)] bg-white/50 px-3 py-1.5 text-[11px] font-medium text-[var(--portal-navy)] transition hover:bg-white/70"
            >
              Collapse
            </button>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {activeStrategy.fields.map((f) => (
              <Field key={String(f.key)} inputs={inputs} setInputs={setInputs} field={f} />
            ))}
          </div>
        </section>
      )}

      {/* D + E. Decision Map + Key Takeaways */}
      <section className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_320px]">
        <div className="portal-glass-panel portal-glass-panel-lifted rounded-2xl p-5">
          <div className="mb-3 flex items-end justify-between gap-3">
            <div>
              <p className="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">Hero visual</p>
              <h2 className="font-serif text-lg font-light text-[var(--portal-navy)]">Decision Map</h2>
            </div>
            <p className="text-xs text-[var(--portal-muted)]">
              Live decision tree with probabilities, outcomes, and present values
            </p>
          </div>
          <div className="overflow-x-auto rounded-xl">
            <div className="mx-auto w-full min-w-[620px] max-w-[700px]">
              <TreeSvg inputs={inputs} model={model} />
            </div>
          </div>
        </div>

        <aside className="portal-glass-panel portal-glass-panel-soft portal-glass-panel-lifted rounded-2xl p-5">
          <p className="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">Read-out</p>
          <h2 className="font-serif text-lg font-light text-[var(--portal-navy)]">Key Takeaways</h2>
          <ul className="mt-3 space-y-2.5">
            {takeaways.map((t, i) => (
              <li key={i} className="flex items-start gap-2 text-sm leading-5">
                <span
                  className="mt-1.5 h-1.5 w-1.5 flex-none rounded-full"
                  style={{
                    backgroundColor:
                      t.tone === "positive"
                        ? "var(--portal-success)"
                        : t.tone === "caution"
                          ? "var(--portal-danger)"
                          : "var(--portal-blue-gray)",
                  }}
                />
                <span className="text-[var(--portal-text)]">{t.text}</span>
              </li>
            ))}
          </ul>
        </aside>
      </section>

      {/* F. Analysis Detail */}
      <section className="portal-glass-panel portal-glass-panel-soft portal-glass-panel-lifted rounded-2xl p-5">
        <div className="mb-3 flex items-end justify-between gap-3">
          <div>
            <p className="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">Evidence</p>
            <h2 className="font-serif text-lg font-light text-[var(--portal-navy)]">Analysis Detail</h2>
          </div>
          <button
            type="button"
            onClick={() => setShowDetail((v) => !v)}
            className="rounded-full border border-[var(--portal-border)] bg-white/50 px-3 py-1.5 text-[11px] font-medium text-[var(--portal-navy)] transition hover:bg-white/70"
          >
            {showDetail ? "Collapse" : "Show All Branches"}
          </button>
        </div>
        {showDetail ? (
          <div className="overflow-x-auto rounded-xl">
            <table className="w-full min-w-[640px] text-left text-sm">
              <thead className="text-[10px] uppercase tracking-wide text-[var(--portal-muted)]">
                <tr>
                  <th className="pb-2">Branch</th>
                  <th>P</th>
                  <th>Price</th>
                  <th>After-tax proceeds</th>
                  <th>PV</th>
                </tr>
              </thead>
              <tbody>
                {model.branches.map((b) => (
                  <tr key={b.id} className="border-t border-[var(--portal-border)]/50">
                    <td className="py-1.5">{b.label}</td>
                    <td>{pct(b.p)}</td>
                    <td>{money(b.price)}</td>
                    <td>{money(b.afterTax)}</td>
                    <td className="font-semibold text-[var(--portal-navy)]">{money(b.pv)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="text-sm text-[var(--portal-muted)]">
            {model.branches.length} branches across {ranking.length} enabled paths · winner {model.winner.short} at{" "}
            {money(model.winner.emvPv)} PV. Open <span className="font-medium text-[var(--portal-navy)]">Show All Branches</span> for the full
            probability, price, after-tax and discounted-PV breakdown.
          </p>
        )}
      </section>
    </div>
  )
}
