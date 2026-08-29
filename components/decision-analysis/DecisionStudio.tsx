"use client"

import { useMemo, useState } from "react"
import { DEFAULT_INPUTS } from "@/lib/decision-analysis/defaults"
import { evaluate, money, pct } from "@/lib/decision-analysis/model"
import { buildDecisionPdf, downloadPdf } from "@/lib/decision-analysis/pdf"
import type { Inputs, OptionId } from "@/lib/decision-analysis/types"
import { TreeSvg } from "./TreeSvg"

type NumKind = "money" | "months" | "pct"
type ParentField = { key: keyof Inputs; label: string; kind: "text" | NumKind }

const PARENT: ParentField[] = [
  { key: "propertyName", label: "Name", kind: "text" },
  { key: "purchasePrice", label: "Purchase", kind: "money" },
  { key: "extraSpent", label: "Extra spent", kind: "money" },
  { key: "contributoryToDate", label: "Contributory extra", kind: "money" },
  { key: "appraisal", label: "Appraisal", kind: "money" },
  { key: "sellingCostPct", label: "Selling costs %", kind: "pct" },
  { key: "discountRate", label: "Discount %", kind: "pct" },
  { key: "taxRate", label: "Tax placeholder %", kind: "pct" },
]

type OptField = { key: keyof Inputs; label: string; kind: NumKind }

const OPTIONS: {
  id: OptionId
  onKey: keyof Inputs
  title: string
  blurb: string
  tone: string
  fields: OptField[]
}[] = [
  {
    id: 1, onKey: "o1On", title: "1  As-is", blurb: "Same house. Extra kit is salvage.", tone: "border-[#1B365D]/20",
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
    id: 2, onKey: "o2On", title: "2  Improve", blurb: "Same product, more brick.", tone: "border-[#0F6E6B]/30",
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
    id: 3, onKey: "o3On", title: "3  Join", blurb: "One ticket: house + business.", tone: "border-[#B85C38]/30",
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
    id: 4, onKey: "o4On", title: "4  Fork", blurb: "House to one buyer, assets to another.", tone: "border-[#6C3483]/30",
    fields: [
      { key: "o4Capex", label: "Split cost", kind: "money" },
      { key: "o4Months", label: "Months", kind: "months" },
      { key: "o4AssetBase", label: "Asset proceeds base", kind: "money" },
      { key: "o4Salvage", label: "Leftover salvage", kind: "money" },
      { key: "o4LowDelta", label: "Low vs bases %", kind: "pct" },
      { key: "o4MidDelta", label: "Mid vs bases %", kind: "pct" },
      { key: "o4HighDelta", label: "High vs bases %", kind: "pct" },
      { key: "o4PLow", label: "P low %", kind: "pct" },
      { key: "o4PMid", label: "P mid %", kind: "pct" },
      { key: "o4PHigh", label: "P high %", kind: "pct" },
    ],
  },
  {
    id: 5, onKey: "o5On", title: "5  Hold", blurb: "Keep or sell only a share.", tone: "border-[#7D6608]/30",
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

function readValue(inputs: Inputs, kind: ParentField["kind"], key: keyof Inputs): string {
  const v = inputs[key]
  if (kind === "text") return String(v)
  if (typeof v === "boolean") return v ? "true" : "false"
  if (kind === "pct") return String(Math.round(Number(v) * 1000) / 10)
  return String(v)
}

function writeValue(kind: ParentField["kind"], raw: string): string | number {
  if (kind === "text") return raw
  const n = Number(raw)
  if (Number.isNaN(n)) return 0
  if (kind === "pct") return n / 100
  return n
}

export function DecisionStudio() {
  const [inputs, setInputs] = useState<Inputs>(DEFAULT_INPUTS)
  const [busy, setBusy] = useState(false)
  const model = useMemo(() => evaluate(inputs), [inputs])

  async function onPdf() {
    setBusy(true)
    try {
      const bytes = await buildDecisionPdf(inputs, model)
      const slug = inputs.propertyName.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")
      downloadPdf(bytes, `${slug || "decision"}-analysis.pdf`)
    } finally {
      setBusy(false)
    }
  }

  const gap = model.runnerUp ? model.winner.emvPv - model.runnerUp.emvPv : 0

  return (
    <div className="min-h-screen bg-[#FBF9F4] text-[#1B365D]">
      <header className="sticky top-0 z-10 border-b border-[#C4A35A]/40 bg-[#1B365D] text-white">
        <div className="mx-auto flex max-w-[1600px] flex-wrap items-center justify-between gap-3 px-5 py-3">
          <div>
            <p className="text-xs uppercase tracking-[0.18em] text-[#C4A35A]">Decision analysis</p>
            <h1 className="text-lg font-semibold">{inputs.propertyName}</h1>
          </div>
          <div className="flex items-center gap-2">
            <button type="button" onClick={() => setInputs(DEFAULT_INPUTS)} className="rounded-full border border-white/30 px-3 py-1.5 text-xs">Reset placeholders</button>
            <button type="button" onClick={onPdf} disabled={busy} className="rounded-full bg-[#C4A35A] px-4 py-1.5 text-xs font-semibold text-[#1B365D] disabled:opacity-60">{busy ? "Building PDF..." : "Download PDF"}</button>
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-[1600px] space-y-5 px-5 py-5">
        <section className="rounded-xl border border-[#1B365D]/10 bg-white p-4 shadow-sm">
          <div className="mb-3 flex items-end justify-between gap-3">
            <div>
              <p className="text-xs uppercase tracking-[0.16em] text-[#C4A35A]">Parent asset</p>
              <h2 className="text-base font-semibold">Shared facts for every path</h2>
            </div>
            <p className="text-xs text-[#5D6D7E]">Sunk {money(inputs.purchasePrice + inputs.extraSpent)} · basis {money(model.basis)}</p>
          </div>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4 xl:grid-cols-8">
            {PARENT.map((f) => (
              <label key={String(f.key)} className="block">
                <span className="mb-0.5 block text-[11px] uppercase tracking-wide text-[#5D6D7E]">{f.label}</span>
                <input className="w-full rounded-md border border-[#1B365D]/15 bg-[#FBF9F4] px-2 py-1.5 text-sm text-blue-700" value={readValue(inputs, f.kind, f.key)} onChange={(e) => setInputs((prev) => ({ ...prev, [f.key]: writeValue(f.kind, e.target.value) }))} />
              </label>
            ))}
          </div>
        </section>

        {model.flags.length > 0 && (
          <div className="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
            {model.flags.map((f) => (<p key={f}>{f}</p>))}
          </div>
        )}

        <section className="rounded-xl border border-[#C4A35A] bg-[#F7F3EA] p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[#C4A35A]">Recommendation</p>
          <h2 className="mt-1 text-2xl font-semibold">{model.winner.name}</h2>
          <p className="mt-1 text-sm text-[#5D6D7E]">
            PV expected net {money(model.winner.emvPv)} in {model.winner.months} months
            {model.runnerUp ? ` · beats ${model.runnerUp.short} by ${money(gap)}` : ""}.
            Toggle a column off if that path is not in play.
          </p>
        </section>

        <section className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-5">
          {OPTIONS.map((opt) => {
            const score = model.scores.find((s) => s.option === opt.id)!
            const on = Boolean(inputs[opt.onKey])
            return (
              <article key={opt.id} className={`rounded-xl border bg-white p-3 shadow-sm ${opt.tone} ${on ? "" : "opacity-55"}`}>
                <div className="mb-2 flex items-start justify-between gap-2">
                  <div>
                    <h3 className="text-sm font-semibold">{opt.title}</h3>
                    <p className="text-[11px] text-[#5D6D7E]">{opt.blurb}</p>
                  </div>
                  <label className="flex items-center gap-1 text-[11px] uppercase tracking-wide">
                    <input type="checkbox" checked={on} onChange={(e) => setInputs((prev) => ({ ...prev, [opt.onKey]: e.target.checked }))} />
                    On
                  </label>
                </div>
                <p className={`mb-3 text-xl font-semibold ${score.best && on ? "text-[#B8860B]" : ""}`}>{on ? money(score.emvPv) : "—"}</p>
                <p className="mb-3 text-[11px] text-[#5D6D7E]">{score.months} mo · future {money(score.futureCash)}{score.best && on ? " · BEST" : ""}</p>
                <div className="space-y-2">
                  {opt.fields.map((f) => (
                    <label key={String(f.key)} className="block">
                      <span className="mb-0.5 block text-[10px] uppercase tracking-wide text-[#5D6D7E]">{f.label}</span>
                      <input disabled={!on} className="w-full rounded-md border border-[#1B365D]/15 bg-[#FBF9F4] px-2 py-1 text-sm text-blue-700 disabled:text-[#5D6D7E]" value={readValue(inputs, f.kind, f.key)} onChange={(e) => setInputs((prev) => ({ ...prev, [f.key]: writeValue(f.kind, e.target.value) }))} />
                    </label>
                  ))}
                </div>
              </article>
            )
          })}
        </section>

        <section className="overflow-x-auto rounded-xl border border-[#1B365D]/10 bg-white p-4 shadow-sm">
          <h2 className="mb-2 text-sm font-semibold">Live tree</h2>
          <TreeSvg inputs={inputs} model={model} />
        </section>

        <section className="overflow-x-auto rounded-xl border border-[#1B365D]/10 bg-white p-4 shadow-sm">
          <h2 className="mb-3 text-sm font-semibold">Branch table</h2>
          <table className="w-full text-left text-sm">
            <thead className="text-xs uppercase tracking-wide text-[#5D6D7E]">
              <tr><th className="pb-2">Branch</th><th>P</th><th>Price</th><th>After tax</th><th>PV</th></tr>
            </thead>
            <tbody>
              {model.branches.map((b) => (
                <tr key={b.id} className="border-t border-[#1B365D]/10">
                  <td className="py-1.5">{b.label}</td>
                  <td>{pct(b.p)}</td>
                  <td>{money(b.price)}</td>
                  <td>{money(b.afterTax)}</td>
                  <td className="font-semibold">{money(b.pv)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      </main>
    </div>
  )
}
