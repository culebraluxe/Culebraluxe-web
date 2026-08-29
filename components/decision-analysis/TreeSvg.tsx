"use client"

import { compactMoney, pct } from "@/lib/decision-analysis/model"
import type { Inputs, ModelResult, OptionId } from "@/lib/decision-analysis/types"

const COL = {
  1: { stroke: "#1B365D", fill: "#D6EAF8" },
  2: { stroke: "#0F6E6B", fill: "#D5F5E3" },
  3: { stroke: "#B85C38", fill: "#F5CBA7" },
  4: { stroke: "#6C3483", fill: "#E8DAEF" },
  5: { stroke: "#7D6608", fill: "#FCF3CF" },
} as const

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
  const n = Math.max(cols.length, 1)
  const colW = 220
  const gap = 16
  const pad = 20
  const width = pad * 2 + n * colW + (n - 1) * gap
  const headerH = 54
  const rowH = 52
  const failH = 52
  const emvY = headerH + 36 + rowH * 3 + failH + 16
  const height = emvY + 40

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="w-full h-auto" role="img" aria-label="Five-path decision comparison">
      <style>{`
        .lbl { font: 700 11px ui-sans-serif, system-ui; }
        .sm { font: 600 10px ui-sans-serif, system-ui; fill: #2C3E50; }
        .tiny { font: 500 9px ui-sans-serif, system-ui; fill: #5D6D7E; }
        .wht { font: 700 10px ui-sans-serif, system-ui; fill: #fff; }
      `}</style>

      {cols.map((s, i) => {
        const x = pad + i * (colW + gap)
        const c = COL[s.option as OptionId]
        const mid = x + colW / 2
        const tips = TIPS[s.option as OptionId]
        const fail = s.option === 3 ? byId["3F"] : undefined
        return (
          <g key={s.option}>
            <rect x={x} y={0} width={colW} height={headerH} rx="6" fill={c.fill} stroke={c.stroke} />
            <text x={mid} y={22} textAnchor="middle" className="lbl" fill={c.stroke}>
              {s.short.toUpperCase()}
            </text>
            <text x={mid} y={40} textAnchor="middle" className="tiny">
              {s.months} months · chance of price
            </text>

            {tips.map(([id, name], ti) => {
              const y = headerH + 20 + ti * rowH
              const br = byId[id]
              if (!br) return null
              return (
                <g key={id}>
                  <rect x={x} y={y} width={colW} height={44} rx="5" fill="#fff" stroke={c.stroke} />
                  <text x={x + 10} y={y + 18} className="lbl" fill={c.stroke}>
                    {pct(br.p)}  {name}
                  </text>
                  <text x={x + 10} y={y + 34} className="sm">
                    {compactMoney(br.price)} list · PV {compactMoney(br.pv)}
                  </text>
                </g>
              )
            })}

            <g>
              {fail ? (
                <>
                  <rect x={x} y={headerH + 20 + 3 * rowH} width={colW} height={44} rx="5" fill="#FADBD8" stroke="#922B21" />
                  <text x={x + 10} y={headerH + 38 + 3 * rowH} className="lbl" fill="#922B21">
                    {pct(fail.p)}  FAIL
                  </text>
                  <text x={x + 10} y={headerH + 54 + 3 * rowH} className="sm">
                    {compactMoney(fail.price)} salvage · PV {compactMoney(fail.pv)}
                  </text>
                </>
              ) : (
                <text x={mid} y={headerH + 48 + 3 * rowH} textAnchor="middle" className="tiny">
                  no second chance node
                </text>
              )}
            </g>

            <rect x={x + 24} y={emvY} width={colW - 48} height={26} rx="13" fill={c.stroke} />
            <text x={mid} y={emvY + 17} textAnchor="middle" className="wht">
              {compactMoney(s.emvPv)}
              {s.best ? "  BEST" : ""}
            </text>
          </g>
        )
      })}
    </svg>
  )
}
