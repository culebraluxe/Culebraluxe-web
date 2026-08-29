"use client"

import { compactMoney, pct } from "@/lib/decision-analysis/model"
import type { Inputs, ModelResult, OptionId } from "@/lib/decision-analysis/types"

const COL: Record<OptionId, { border: string; bg: string; text: string; pill: string }> = {
  1: { border: "border-[#1B365D]", bg: "bg-[#D6EAF8]", text: "text-[#1B365D]", pill: "bg-[#1B365D]" },
  2: { border: "border-[#0F6E6B]", bg: "bg-[#D5F5E3]", text: "text-[#0F6E6B]", pill: "bg-[#0F6E6B]" },
  3: { border: "border-[#B85C38]", bg: "bg-[#F5CBA7]", text: "text-[#B85C38]", pill: "bg-[#B85C38]" },
  4: { border: "border-[#6C3483]", bg: "bg-[#E8DAEF]", text: "text-[#6C3483]", pill: "bg-[#6C3483]" },
  5: { border: "border-[#7D6608]", bg: "bg-[#FCF3CF]", text: "text-[#7D6608]", pill: "bg-[#7D6608]" },
}

const TIPS: Record<OptionId, [string, string][]> = {
  1: [["1L", "LOW"], ["1M", "MID"], ["1H", "IDEAL"]],
  2: [["2L", "LOW"], ["2M", "BASE"], ["2H", "HIGH"]],
  3: [["3L", "LOW"], ["3M", "BASE"], ["3H", "HIGH"]],
  4: [["4L", "LOW"], ["4M", "MID"], ["4H", "HIGH"]],
  5: [["5L", "LOW"], ["5M", "MID"], ["5H", "HIGH"]],
}

export function TreeSvg({ model }: { inputs: Inputs; model: ModelResult }) {
  const byId = Object.fromEntries(model.branches.map((x) => [x.id, x]))
  const cols = model.scores.filter((s) => s.on)

  if (cols.length === 0) {
    return <p className="text-sm text-[#5D6D7E]">Turn on at least one path to see the tree.</p>
  }

  return (
    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-5">
      {cols.map((s) => {
        const c = COL[s.option as OptionId]
        const fail = s.option === 3 ? byId["3F"] : undefined
        return (
          <div key={s.option} className={`rounded-xl border ${c.border} bg-white p-2`}>
            <div className={`rounded-lg ${c.bg} px-3 py-2 text-center ${c.text}`}>
              <p className="text-xs font-semibold tracking-wide">{s.short.toUpperCase()}</p>
              <p className="text-[11px] text-[#5D6D7E]">{s.months} months</p>
            </div>
            <div className="mt-2 space-y-2">
              {TIPS[s.option as OptionId].map(([id, name]) => {
                const br = byId[id]
                if (!br) return null
                return (
                  <div key={id} className={`rounded-lg border ${c.border} px-3 py-2`}>
                    <p className={`text-xs font-semibold ${c.text}`}>
                      {pct(br.p)}  {name}
                    </p>
                    <p className="text-[11px] text-[#2C3E50]">
                      {compactMoney(br.price)} list · PV {compactMoney(br.pv)}
                    </p>
                  </div>
                )
              })}
              {fail ? (
                <div className="rounded-lg border border-[#922B21] bg-[#FADBD8] px-3 py-2">
                  <p className="text-xs font-semibold text-[#922B21]">
                    {pct(fail.p)}  FAIL
                  </p>
                  <p className="text-[11px] text-[#2C3E50]">
                    {compactMoney(fail.price)} salvage · PV {compactMoney(fail.pv)}
                  </p>
                </div>
              ) : (
                <div className="rounded-lg border border-dashed border-[#1B365D]/20 px-3 py-2 text-center text-[11px] text-[#5D6D7E]">
                  no second chance node
                </div>
              )}
            </div>
            <div className={`mt-3 rounded-full ${c.pill} px-3 py-1.5 text-center text-xs font-semibold text-white`}>
              {compactMoney(s.emvPv)}{s.best ? "  BEST" : ""}
            </div>
          </div>
        )
      })}
    </div>
  )
}
