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

function tipsFor(option: OptionId): [string, string][] {
  if (option === 1) return [["1L", "LOW"], ["1M", "MID"], ["1H", "IDEAL"]]
  if (option === 2) return [["2L", "LOW"], ["2M", "BASE"], ["2H", "HIGH"]]
  if (option === 3) return [["3L", "LOW"], ["3M", "BASE"], ["3H", "HIGH"], ["3F", "FAIL"]]
  if (option === 4) return [["4L", "LOW"], ["4M", "MID"], ["4H", "HIGH"]]
  return [["5L", "LOW"], ["5M", "MID"], ["5H", "HIGH"]]
}

export function TreeSvg({ model }: { inputs: Inputs; model: ModelResult }) {
  const b = Object.fromEntries(model.branches.map((x) => [x.id, x]))
  const cols = model.scores.filter((s) => s.on)

  if (cols.length === 0) {
    return <p className="text-sm text-[#5D6D7E]">Turn on at least one path to see the tree.</p>
  }

  const rowH = 52
  const pathGap = 22
  let y = 16
  const layout = cols.map((s) => {
    const tips = tipsFor(s.option as OptionId)
    const h = Math.max(72, tips.length * rowH)
    const y0 = y
    y += h + pathGap
    return { s, tips, y0, h, midY: y0 + h / 2 }
  })

  const height = Math.max(y + 8, 280)
  const xDecide = 16
  const xOpt = 150
  const optW = 132
  const xChance = 318
  const xBox = 400
  const boxW = 210
  const xEmv = 630
  const emvW = 118
  const width = 764
  const decideY = height / 2 - 22

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      width={width}
      height={height}
      role="img"
      aria-label="Decision tree"
      style={{ width: "100%", height: "auto", minHeight: 280, display: "block" }}
    >
      <style>{`
        .lbl { font: 700 11px ui-sans-serif, system-ui; }
        .sm { font: 600 10px ui-sans-serif, system-ui; fill: #2C3E50; }
        .tiny { font: 600 9px ui-sans-serif, system-ui; fill: #5D6D7E; }
        .wht { font: 700 10px ui-sans-serif, system-ui; fill: #fff; }
        .edge { font: 700 10px ui-sans-serif, system-ui; }
      `}</style>

      <rect x={xDecide} y={decideY} width="88" height="44" rx="4" fill="#1B365D" />
      <text x={xDecide + 44} y={decideY + 18} textAnchor="middle" className="wht">DECIDE</text>
      <text x={xDecide + 44} y={decideY + 32} textAnchor="middle" className="wht">now</text>

      {layout.map(({ s, tips, y0, midY }) => {
        const c = COL[s.option as OptionId]
        const optX = xOpt
        const chanceY = midY
        return (
          <g key={s.option}>
            <line x1={xDecide + 88} y1={decideY + 22} x2={optX} y2={midY} stroke={c.stroke} strokeWidth="1.6" />
            <rect x={optX} y={midY - 20} width={optW} height="40" rx="4" fill={c.fill} stroke={c.stroke} />
            <text x={optX + optW / 2} y={midY - 4} textAnchor="middle" className="lbl" fill={c.stroke}>
              {s.short.toUpperCase()}
            </text>
            <text x={optX + optW / 2} y={midY + 12} textAnchor="middle" className="tiny">
              {s.months} mo
            </text>

            <line x1={optX + optW} y1={midY} x2={xChance - 16} y2={chanceY} stroke={c.stroke} strokeWidth="1.4" />
            <circle cx={xChance} cy={chanceY} r="14" fill={c.fill} stroke={c.stroke} />
            <text x={xChance} y={chanceY + 3} textAnchor="middle" className="tiny">P</text>

            {tips.map(([id, name], ti) => {
              const br = b[id]
              if (!br) return null
              const yy = y0 + ti * rowH + 4
              const boxMid = yy + 20
              return (
                <g key={id}>
                  <line x1={xChance + 14} y1={chanceY} x2={xBox} y2={boxMid} stroke={c.stroke} strokeWidth="1.2" />
                  <text
                    x={(xChance + 14 + xBox) / 2}
                    y={(chanceY + boxMid) / 2 - 4}
                    textAnchor="middle"
                    className="edge"
                    fill={c.stroke}
                  >
                    {pct(br.p)}
                  </text>
                  <rect x={xBox} y={yy} width={boxW} height="40" rx="4" fill="#fff" stroke={c.stroke} />
                  <text x={xBox + 8} y={yy + 16} className="lbl" fill={c.stroke}>{name}</text>
                  <text x={xBox + 8} y={yy + 32} className="sm">
                    {compactMoney(br.price)} · PV {compactMoney(br.pv)}
                  </text>
                </g>
              )
            })}

            <rect x={xEmv} y={midY - 13} width={emvW} height="26" rx="13" fill={c.stroke} />
            <text x={xEmv + emvW / 2} y={midY + 4} textAnchor="middle" className="wht">
              {compactMoney(s.emvPv)}{s.best ? " BEST" : ""}
            </text>
          </g>
        )
      })}
    </svg>
  )
}
