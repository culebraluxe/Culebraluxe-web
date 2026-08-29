"use client"

import { useMemo, useState } from "react"
import { DEFAULT_INPUTS } from "@/lib/decision-analysis/defaults"
import { evaluate, money, pct } from "@/lib/decision-analysis/model"
import { buildDecisionPdf, downloadPdf } from "@/lib/decision-analysis/pdf"
import type { Inputs } from "@/lib/decision-analysis/types"
import { TreeSvg } from "./TreeSvg"

type Field =
  | { key: keyof Inputs; label: string; kind: "money" | "months" | "pct" | "text" }

const GROUPS: { title: string; fields: Field[] }[] = [
  {
    title: "Property",
    fields: [
      { key: "propertyName", label: "Name", kind: "text" },
      { key: "purchasePrice", label: "Purchase price", kind: "money" },
      { key: "extraSpent", label: "Extra spent to date", kind: "money" },
      { key: "contributoryToDate", label: "Contributory share of extra", kind: "money" },
      { key: "appraisal", label: "As-is appraisal", kind: "money" },
      { key: "sellingCostPct", label: "Selling costs", kind: "pct" },
      { key: "discountRate", label: "Discount rate", kind: "pct" },
      { key: "taxRate", label: "Tax rate placeholder", kind: "pct" },
    ],
  },
  {
    title: "Option 1 \u2014 sell as-is",
    fields: [
      { key: "o1Months", label: "Months to close", kind: "months" },
      { key: "o1Salvage", label: "Pod salvage", kind: "money" },
      { key: "o1LowDelta", label: "Low vs appraisal", kind: "pct" },
      { key: "o1MidDelta", label: "Mid vs appraisal", kind: "pct" },
      { key: "o1HighDelta", label: "Ideal vs appraisal", kind: "pct" },
      { key: "o1PLow", label: "P(low)", kind: "pct" },
      { key: "o1PMid", label: "P(mid)", kind: "pct" },
      { key: "o1PHigh", label: "P(ideal)", kind: "pct" },
    ],
  },
  {
    title: "Option 2 \u2014 improve then sell",
    fields: [
      { key: "o2Capex", label: "Remaining capex", kind: "money" },
      { key: "o2Months", label: "Months", kind: "months" },
      { key: "o2Recovery", label: "Capex recovery", kind: "pct" },
      { key: "o2Salvage", label: "Salvage", kind: "money" },
      { key: "o2LowDelta", label: "Low vs improved base", kind: "pct" },
      { key: "o2MidDelta", label: "Base vs improved", kind: "pct" },
      { key: "o2HighDelta", label: "High vs improved", kind: "pct" },
      { key: "o2PLow", label: "P(low)", kind: "pct" },
      { key: "o2PMid", label: "P(base)", kind: "pct" },
      { key: "o2PHigh", label: "P(high)", kind: "pct" },
    ],
  },
  {
    title: "Option 3 \u2014 launch then sell package",
    fields: [
      { key: "o3Capex", label: "Launch cash still required", kind: "money" },
      { key: "o3Months", label: "Months", kind: "months" },
      { key: "o3Noi", label: "Stabilized NOI", kind: "money" },
      { key: "o3CapRate", label: "Exit cap rate", kind: "pct" },
      { key: "o3PSuccess", label: "P(stabilizes)", kind: "pct" },
      { key: "o3FailSalvage", label: "Fail salvage price", kind: "money" },
      { key: "o3LowDelta", label: "Low vs NOI/cap", kind: "pct" },
      { key: "o3MidDelta", label: "Base vs NOI/cap", kind: "pct" },
      { key: "o3HighDelta", label: "High vs NOI/cap", kind: "pct" },
      { key: "o3PLow", label: "P(low | success)", kind: "pct" },
      { key: "o3PMid", label: "P(base | success)", kind: "pct" },
      { key: "o3PHigh", label: "P(high | success)", kind: "pct" },
    ],
  },
]

function readValue(inputs: Inputs, field: Field): string {
  const v = inputs[field.key]
  if (field.kind === "text") return String(v)
  if (field.kind === "pct") return String(Math.round(Number(v) * 1000) / 10)
  return String(v)
}

function writeValue(field: Field, raw: string): Inputs[keyof Inputs] {
  if (field.kind === "text") return raw
  const n = Number(raw)
  if (Number.isNaN(n)) return 0
  if (field.kind === "pct") return n / 100
  return n
}

export function DecisionStudio() {
  const [inputs, setInputs] = useState<Inputs>(DEFAULT_INPUTS)
  const [busy, setBusy] = useState(false)
  const model = useMemo(() => evaluate(inputs), [inputs])

  function setField(field: Field, raw: string) {
    setInputs((prev) => ({ ...prev, [field.key]: writeValue(field, raw) }))
  }

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

  return (
    <div className="min-h-screen bg-[#FBF9F4] text-[#1B365D]">
      <header className="sticky top-0 z-10 border-b border-[#C4A35A]/40 bg-[#1B365D] text-white">
        <div className="mx-auto flex max-w-7xl flex-wrap items-center justify-between gap-3 px-5 py-3">
          <div>
            <p className="text-xs uppercase tracking-[0.18em] text-[#C4A35A]">Decision analysis</p>
            <h1 className="text-lg font-semibold">{inputs.propertyName}</h1>
          </div>
          <div className="flex items-center gap-2">
            <button type="button" onClick={() => setInputs(DEFAULT_INPUTS)} className="rounded-full border border-white/30 px-3 py-1.5 text-xs">
              Reset placeholders
            </button>
            <button type="button" onClick={onPdf} disabled={busy} className="rounded-full bg-[#C4A35A] px-4 py-1.5 text-xs font-semibold text-[#1B365D] disabled:opacity-60">
              {busy ? "Building PDF..." : "Download PDF"}
            </button>
          </div>
        </div>
      </header>

      <main className="mx-auto grid max-w-7xl gap-6 px-5 py-6 lg:grid-cols-[320px_1fr]">
        <aside className="space-y-4">
          {GROUPS.map((g) => (
            <section key={g.title} className="rounded-xl border border-[#1B365D]/10 bg-white p-4 shadow-sm">
              <h2 className="mb-3 text-sm font-semibold">{g.title}</h2>
              <div className="space-y-2">
                {g.fields.map((f) => (
                  <label key={String(f.key)} className="block">
                    <span className="mb-0.5 block text-[11px] uppercase tracking-wide text-[#5D6D7E]">
                      {f.label}
                      {f.kind === "pct" ? " (%)" : ""}
                    </span>
                    <input
                      className="w-full rounded-md border border-[#1B365D]/15 bg-[#FBF9F4] px-2 py-1.5 text-sm text-blue-700"
                      value={readValue(inputs, f)}
                      onChange={(e) => setField(f, e.target.value)}
                    />
                  </label>
                ))}
              </div>
            </section>
          ))}
        </aside>

        <section className="space-y-5">
          {model.flags.length > 0 && (
            <div className="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
              {model.flags.map((f) => (
                <p key={f}>{f}</p>
              ))}
            </div>
          )}

          <div className="rounded-xl border border-[#1B365D]/10 bg-white p-5 shadow-sm">
            <p className="text-xs uppercase tracking-[0.16em] text-[#C4A35A]">Recommendation</p>
            <h2 className="mt-1 text-2xl font-semibold">{model.winner.name}</h2>
            <p className="mt-1 text-sm text-[#5D6D7E]">
              PV expected net {money(model.winner.emvPv)} in {model.winner.months} months.
              Sunk {money(inputs.purchasePrice + inputs.extraSpent)} does not decide the next path.
            </p>
            <div className="mt-4 grid gap-3 sm:grid-cols-3">
              {model.scores.map((s) => (
                <div key={s.option} className={`rounded-lg border p-3 ${s.best ? "border-[#C4A35A] bg-[#F7F3EA]" : "border-[#1B365D]/10"}`}>
                  <p className="text-xs uppercase tracking-wide text-[#5D6D7E]">{s.name}</p>
                  <p className="mt-1 text-xl font-semibold">{money(s.emvPv)}</p>
                  <p className="text-xs text-[#5D6D7E]">
                    {s.months} mo \u00b7 future cash {money(s.futureCash)}
                    {s.best ? " \u00b7 BEST" : ""}
                  </p>
                </div>
              ))}
            </div>
          </div>

          <div className="overflow-x-auto rounded-xl border border-[#1B365D]/10 bg-white p-4 shadow-sm">
            <h2 className="mb-2 text-sm font-semibold">Live tree</h2>
            <TreeSvg inputs={inputs} model={model} />
          </div>

          <div className="overflow-x-auto rounded-xl border border-[#1B365D]/10 bg-white p-4 shadow-sm">
            <h2 className="mb-3 text-sm font-semibold">Branch table</h2>
            <table className="w-full text-left text-sm">
              <thead className="text-xs uppercase tracking-wide text-[#5D6D7E]">
                <tr>
                  <th className="pb-2">Branch</th>
                  <th>P</th>
                  <th>Price</th>
                  <th>After tax</th>
                  <th>PV</th>
                </tr>
              </thead>
              <tbody>
                {model.branches.map((b) => (
                  <tr key={b.id} className="border-t border-[#1B365D]/8">
                    <td className="py-1.5">{b.label}</td>
                    <td>{pct(b.p)}</td>
                    <td>{money(b.price)}</td>
                    <td>{money(b.afterTax)}</td>
                    <td className="font-semibold">{money(b.pv)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      </main>
    </div>
  )
}
