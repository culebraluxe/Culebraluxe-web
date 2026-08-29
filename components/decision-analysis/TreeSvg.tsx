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

export function TreeSvg({ model }: { inputs: Inputs; model: ModelResult }) {
  const b = Object.fromEntries(model.branches.map((x) => [x.id, x]))
  const cols = model.scores.filter((s) => s.on)
  const width = Math.max(1100, 180 + Math.max(cols.length, 1) * 230)
  const height = 420

  if (cols.length === 0) {
    return <p className="text-sm text-[#5D6D7E]">Turn on at least one path to see the tree.</p>
  }

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
        .tiny { font: 500 9px ui-sans-serif, system-ui; fill: #5D6D7E; }
        .wht { font: 700 10px ui-sans-serif, system-ui; fill: #fff; }
      `}</style>
      <rect x="12" y="188" width="88" height="44" rx="4" fill="#1B365D" />
      <text x="56" y="206" textAnchor="middle" className="wht">DECIDE</text>
      <text x="56" y="220" textAnchor="middle" className="wht">now</text>

      {cols.map((s, i) => {
        const x = 160 + i * 230
        const c = COL[s.option as OptionId]
        const tips =
          s.option === 3
            ? [["3L", "LOW"], ["3M", "BASE"], ["3H", "HIGH"], ["3F", "FAIL"]]
            : s.option === 1
              ? [["1L", "LOW"], ["1M", "MID"], ["1H", "IDEAL"]]
              : s.option === 2
                ? [["2L", "LOW"], ["2M", "BASE"], ["2H", "HIGH"]]
                : s.option === 4
                  ? [["4L", "LOW"], ["4M", "MID"], ["4H", "HIGH"]]
                  : [["5L", "LOW"], ["5M", "MID"], ["5H", "HIGH"]]
        return (
          <g key={s.option}>
            <line x1="100" y1="210" x2={x + 70} y2="58" stroke={c.stroke} strokeWidth="1.4" />
            <rect x={x} y="20" width="140" height="40" rx="4" fill={c.fill} stroke={c.stroke} />
            <text x={x + 70} y="36" textAnchor="middle" className="lbl" fill={c.stroke}>{s.short.toUpperCase()}</text>
            <text x={x + 70} y="52" textAnchor="middle" className="tiny">{s.months} mo</text>
            <circle cx={x + 70} cy="84" r="16" fill={c.fill} stroke={c.stroke} />
            <text x={x + 70} y="88" textAnchor="middle" className="tiny">chance</text>
            {tips.map(([id, name], ti) => {
              const yy = 118 + ti * 58
              const br = b[id]
              if (!br) return null
              return (
                <g key={id}>
                  <line x1={x + 70} y1="100" x2={x + 70} y2={yy - 16} stroke={c.stroke} />
                  <rect x={x} y={yy - 16} width="200" height="44" rx="4" fill="#fff" stroke={c.stroke} />
                  <text x={x + 8} y={yy + 2} className="lbl" fill={c.stroke}>{pct(br.p)}  {name}</text>
                  <text x={x + 8} y={yy + 18} className="sm">{compactMoney(br.price)} · PV {compactMoney(br.pv)}</text>
                </g>
              )
            })}
            <rect x={x} y="368" width="140" height="26" rx="13" fill={c.stroke} />
            <text x={x + 70} y="385" textAnchor="middle" className="wht">{compactMoney(s.emvPv)}{s.best ? " BEST" : ""}</text>
          </g>
        )
      })}
    </svg>
  )
}
