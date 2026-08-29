"use client"

import { compactMoney, pct } from "@/lib/decision-analysis/model"
import type { Inputs, ModelResult } from "@/lib/decision-analysis/types"

const C = {
  navy: "#1B365D",
  teal: "#0F6E6B",
  clay: "#B85C38",
  pale1: "#D6EAF8",
  pale2: "#D5F5E3",
  pale3: "#F5CBA7",
  fail: "#FADBD8",
  slate: "#2C3E50",
}

export function TreeSvg({ inputs, model }: { inputs: Inputs; model: ModelResult }) {
  const b = Object.fromEntries(model.branches.map((x) => [x.id, x]))
  return (
    <svg viewBox="0 0 1100 640" className="w-full h-auto" role="img" aria-label="Decision tree">
      <style>{`
        .lbl { font: 700 11px ui-sans-serif, system-ui; fill: ${C.navy}; }
        .sm { font: 600 10px ui-sans-serif, system-ui; fill: ${C.slate}; }
        .tiny { font: 500 9px ui-sans-serif, system-ui; fill: #5D6D7E; }
        .wht { font: 700 10px ui-sans-serif, system-ui; fill: #fff; }
      `}</style>

      <rect x="16" y="290" width="88" height="44" rx="4" fill={C.navy} />
      <text x="60" y="308" textAnchor="middle" className="wht">DECIDE</text>
      <text x="60" y="322" textAnchor="middle" className="wht">now</text>

      <line x1="104" y1="312" x2="180" y2="90" stroke={C.navy} strokeWidth="1.6" />
      <text x="118" y="180" className="tiny">no new cash</text>
      <rect x="180" y="68" width="130" height="44" rx="4" fill={C.pale1} stroke={C.navy} />
      <text x="245" y="86" textAnchor="middle" className="lbl">1  SELL AS-IS</text>
      <text x="245" y="102" textAnchor="middle" className="sm">{inputs.o1Months} months</text>
      <line x1="310" y1="90" x2="360" y2="90" stroke={C.navy} />
      <circle cx="380" cy="90" r="22" fill={C.pale1} stroke={C.navy} strokeWidth="1.4" />
      <text x="380" y="88" textAnchor="middle" className="tiny">chance</text>
      <text x="380" y="100" textAnchor="middle" className="tiny">price</text>

      {[
        { y: 28, id: "1L", p: inputs.o1PLow, name: "LOW" },
        { y: 90, id: "1M", p: inputs.o1PMid, name: "MID" },
        { y: 152, id: "1H", p: inputs.o1PHigh, name: "IDEAL" },
      ].map((n) => (
        <g key={n.id}>
          <line x1="402" y1="90" x2="450" y2={n.y} stroke={C.navy} strokeWidth="1" />
          <rect x="450" y={n.y - 20} width="250" height="40" rx="4" fill="#fff" stroke={C.navy} />
          <text x="462" y={n.y - 4} className="lbl">{pct(n.p)}  {n.name}</text>
          <text x="462" y={n.y + 12} className="sm">{compactMoney(b[n.id].price)} list  ·  PV {compactMoney(b[n.id].pv)}</text>
        </g>
      ))}
      <rect x="720" y="70" width="140" height="28" rx="14" fill={C.navy} />
      <text x="790" y="88" textAnchor="middle" className="wht">EMV1 {compactMoney(model.scores[0].emvPv)}{model.scores[0].best ? "  BEST" : ""}</text>

      <line x1="104" y1="312" x2="180" y2="312" stroke={C.teal} strokeWidth="1.6" />
      <text x="118" y="300" className="tiny">+ capex</text>
      <rect x="180" y="290" width="130" height="44" rx="4" fill={C.pale2} stroke={C.teal} />
      <text x="245" y="308" textAnchor="middle" className="lbl">2  IMPROVE</text>
      <text x="245" y="324" textAnchor="middle" className="sm">{inputs.o2Months} months</text>
      <line x1="310" y1="312" x2="360" y2="312" stroke={C.teal} />
      <circle cx="380" cy="312" r="22" fill={C.pale2} stroke={C.teal} strokeWidth="1.4" />
      <text x="380" y="310" textAnchor="middle" className="tiny">chance</text>
      <text x="380" y="322" textAnchor="middle" className="tiny">price</text>
      {[
        { y: 250, id: "2L", p: inputs.o2PLow, name: "LOW" },
        { y: 312, id: "2M", p: inputs.o2PMid, name: "BASE" },
        { y: 374, id: "2H", p: inputs.o2PHigh, name: "HIGH" },
      ].map((n) => (
        <g key={n.id}>
          <line x1="402" y1="312" x2="450" y2={n.y} stroke={C.teal} strokeWidth="1" />
          <rect x="450" y={n.y - 20} width="250" height="40" rx="4" fill="#fff" stroke={C.teal} />
          <text x="462" y={n.y - 4} className="lbl">{pct(n.p)}  {n.name}</text>
          <text x="462" y={n.y + 12} className="sm">{compactMoney(b[n.id].price)} list  ·  PV {compactMoney(b[n.id].pv)}</text>
        </g>
      ))}
      <rect x="720" y="298" width="140" height="28" rx="14" fill={C.teal} />
      <text x="790" y="316" textAnchor="middle" className="wht">EMV2 {compactMoney(model.scores[1].emvPv)}{model.scores[1].best ? "  BEST" : ""}</text>

      <line x1="104" y1="312" x2="180" y2="500" stroke={C.clay} strokeWidth="1.6" />
      <text x="108" y="420" className="tiny">+ launch cash</text>
      <rect x="180" y="478" width="130" height="44" rx="4" fill={C.pale3} stroke={C.clay} />
      <text x="245" y="496" textAnchor="middle" className="lbl">3  LAUNCH</text>
      <text x="245" y="512" textAnchor="middle" className="sm">{inputs.o3Months} months</text>
      <line x1="310" y1="500" x2="358" y2="500" stroke={C.clay} />
      <circle cx="380" cy="500" r="24" fill={C.pale3} stroke={C.clay} strokeWidth="1.4" />
      <text x="380" y="496" textAnchor="middle" className="tiny">does it</text>
      <text x="380" y="508" textAnchor="middle" className="tiny">stabilize?</text>

      <line x1="404" y1="490" x2="458" y2="454" stroke={C.clay} />
      <text x="408" y="468" className="tiny">{pct(inputs.o3PSuccess)} yes</text>
      <circle cx="478" cy="450" r="18" fill={C.pale3} stroke={C.clay} />
      <text x="478" y="448" textAnchor="middle" className="tiny">buyer</text>
      <text x="478" y="459" textAnchor="middle" className="tiny">price</text>

      {[
        { y: 410, id: "3L", name: "LOW" },
        { y: 450, id: "3M", name: "BASE" },
        { y: 490, id: "3H", name: "HIGH" },
      ].map((n) => (
        <g key={n.id}>
          <line x1="496" y1="450" x2="540" y2={n.y} stroke={C.clay} strokeWidth="1" />
          <rect x="540" y={n.y - 18} width="260" height="36" rx="4" fill="#fff" stroke={C.clay} />
          <text x="550" y={n.y - 2} className="lbl">{n.name}  joint {pct(b[n.id].p)}</text>
          <text x="550" y={n.y + 12} className="sm">{compactMoney(b[n.id].price)}  ·  PV {compactMoney(b[n.id].pv)}</text>
        </g>
      ))}

      <line x1="380" y1="524" x2="380" y2="575" stroke="#922B21" />
      <line x1="380" y1="575" x2="540" y2="575" stroke="#922B21" />
      <text x="388" y="548" className="tiny">{pct(1 - inputs.o3PSuccess)} fail</text>
      <rect x="540" y="557" width="260" height="36" rx="4" fill={C.fail} stroke="#922B21" />
      <text x="550" y="573" className="lbl">FAIL  joint {pct(b["3F"].p)}</text>
      <text x="550" y="587" className="sm">{compactMoney(b["3F"].price)} salvage  ·  PV {compactMoney(b["3F"].pv)}</text>

      <rect x="820" y="434" width="140" height="28" rx="14" fill={C.clay} />
      <text x="890" y="452" textAnchor="middle" className="wht">EMV3 {compactMoney(model.scores[2].emvPv)}{model.scores[2].best ? "  BEST" : ""}</text>
    </svg>
  )
}
