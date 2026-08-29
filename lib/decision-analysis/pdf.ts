"use client"

import { PDFDocument, StandardFonts, rgb } from "pdf-lib"
import { compactMoney, money, pct } from "./model"
import type { Inputs, ModelResult } from "./types"

const navy = rgb(0.106, 0.212, 0.365)
const gold = rgb(0.769, 0.639, 0.353)
const slate = rgb(0.173, 0.243, 0.314)
const muted = rgb(0.365, 0.427, 0.494)
const cream = rgb(0.969, 0.953, 0.918)
const white = rgb(1, 1, 1)

function wrap(text: string, max: number) {
  const words = text.split(/\s+/)
  const lines: string[] = []
  let cur = ""
  for (const w of words) {
    const next = cur ? `${cur} ${w}` : w
    if (next.length > max) {
      if (cur) lines.push(cur)
      cur = w
    } else cur = next
  }
  if (cur) lines.push(cur)
  return lines
}

export async function buildDecisionPdf(inputs: Inputs, model: ModelResult): Promise<Uint8Array> {
  const doc = await PDFDocument.create()
  const font = await doc.embedFont(StandardFonts.TimesRoman)
  const bold = await doc.embedFont(StandardFonts.TimesRomanBold)
  const italic = await doc.embedFont(StandardFonts.TimesRomanItalic)
  const page = doc.addPage([612, 792])
  const W = 612
  const H = 792

  page.drawRectangle({ x: 0, y: H - 58, width: W, height: 58, color: navy })
  page.drawRectangle({ x: 0, y: H - 62, width: W, height: 4, color: gold })
  page.drawText("Executive summary  -  five-path exit", { x: 36, y: H - 32, size: 16, font: bold, color: white })
  page.drawText(inputs.propertyName, { x: 36, y: H - 48, size: 9, font: italic, color: rgb(0.84, 0.85, 0.86) })
  page.drawRectangle({ x: 0, y: 0, width: W, height: 28, color: navy })
  page.drawText("As-is / Improve / Join / Fork / Hold  -  not an appraisal", { x: 36, y: 11, size: 8, font, color: white })

  let y = H - 88
  page.drawRectangle({ x: 36, y: y - 48, width: W - 72, height: 52, color: rgb(0.918, 0.949, 0.973), borderColor: navy, borderWidth: 1 })
  page.drawText(`Recommendation:  ${model.winner.name}`, { x: 48, y: y - 16, size: 11, font: bold, color: navy })
  wrap(`PV expected net ${money(model.winner.emvPv)} in about ${model.winner.months} months.`, 92).forEach((line, i) => {
    page.drawText(line, { x: 48, y: y - 32 - i * 11, size: 9, font, color: slate })
  })
  y -= 78

  page.drawText("Path scores", { x: 36, y, size: 12, font: bold, color: navy })
  y -= 8
  const heads = ["Path", "On", "Mo", "Future cash", "PV EMV"]
  const cols = [36, 230, 280, 330, 460]
  page.drawRectangle({ x: 36, y: y - 14, width: W - 72, height: 16, color: navy })
  heads.forEach((h, i) => page.drawText(h, { x: cols[i], y: y - 10, size: 8, font: bold, color: white }))
  y -= 14
  model.scores.forEach((s, i) => {
    y -= 14
    if (i % 2 === 0) page.drawRectangle({ x: 36, y: y - 3, width: W - 72, height: 14, color: cream })
    page.drawText(s.name, { x: cols[0], y, size: 8, font: s.best ? bold : font, color: slate })
    page.drawText(s.on ? "yes" : "no", { x: cols[1], y, size: 8, font, color: slate })
    page.drawText(String(s.months), { x: cols[2], y, size: 8, font, color: slate })
    page.drawText(money(s.futureCash), { x: cols[3], y, size: 8, font, color: slate })
    page.drawText(`${money(s.emvPv)}${s.best ? "  BEST" : ""}`, { x: cols[4], y, size: 8, font: bold, color: slate })
  })

  y -= 28
  page.drawText("Parent inputs", { x: 36, y, size: 12, font: bold, color: navy })
  y -= 6
  const rows: [string, string][] = [
    ["Appraisal", money(inputs.appraisal)],
    ["Purchase + extra", `${money(inputs.purchasePrice)} + ${money(inputs.extraSpent)}`],
    ["Contributory extra", money(inputs.contributoryToDate)],
    ["Selling / discount / tax", `${pct(inputs.sellingCostPct)} / ${pct(inputs.discountRate)} / ${pct(inputs.taxRate)}`],
    ["Join NOI / cap / P(success)", `${money(inputs.o3Noi)} / ${pct(inputs.o3CapRate)} / ${pct(inputs.o3PSuccess)}`],
    ["Fork asset base", money(inputs.o4AssetBase)],
    ["Hold share / period cash", `${pct(inputs.o5Share)} / ${money(inputs.o5PeriodCash)}`],
    ["Tax basis used", money(model.basis)],
  ]
  page.drawRectangle({ x: 36, y: y - 14, width: W - 72, height: 16, color: navy })
  page.drawText("Item", { x: 42, y: y - 10, size: 8, font: bold, color: white })
  page.drawText("Value", { x: 320, y: y - 10, size: 8, font: bold, color: white })
  y -= 14
  rows.forEach(([a, b], i) => {
    y -= 13
    if (i % 2 === 0) page.drawRectangle({ x: 36, y: y - 3, width: W - 72, height: 13, color: cream })
    page.drawText(a, { x: 42, y, size: 8, font, color: slate })
    page.drawText(b, { x: 320, y, size: 8, font: bold, color: slate })
  })
  y -= 24
  page.drawText("Disclaimer", { x: 36, y, size: 10, font: bold, color: navy })
  y -= 12
  wrap("Working model for a meeting, not a valuation, appraisal, or tax opinion.", 98).forEach((line) => {
    page.drawText(line, { x: 36, y, size: 8, font: italic, color: muted })
    y -= 11
  })

  const p2 = doc.addPage([792, 612])
  p2.drawRectangle({ x: 0, y: 560, width: 792, height: 52, color: navy })
  p2.drawRectangle({ x: 0, y: 556, width: 792, height: 4, color: gold })
  p2.drawText("Branches  -  probabilities and fold-back values", { x: 36, y: 584, size: 15, font: bold, color: white })
  p2.drawRectangle({ x: 0, y: 0, width: 792, height: 24, color: navy })
  p2.drawText("2 / 2", { x: 744, y: 9, size: 8, font, color: white })

  const h2 = ["Branch", "P", "Gross price", "After-tax net", "PV"]
  const c2 = [36, 280, 360, 500, 640]
  let ty = 532
  p2.drawRectangle({ x: 36, y: ty - 4, width: 720, height: 16, color: navy })
  h2.forEach((h, i) => p2.drawText(h, { x: c2[i], y: ty, size: 8, font: bold, color: white }))
  ty -= 18
  model.branches.forEach((b, i) => {
    if (i % 2 === 0) p2.drawRectangle({ x: 36, y: ty - 3, width: 720, height: 14, color: cream })
    p2.drawText(b.label, { x: c2[0], y: ty, size: 8, font, color: slate })
    p2.drawText(pct(b.p), { x: c2[1], y: ty, size: 8, font, color: slate })
    p2.drawText(money(b.price), { x: c2[2], y: ty, size: 8, font, color: slate })
    p2.drawText(money(b.afterTax), { x: c2[3], y: ty, size: 8, font, color: slate })
    p2.drawText(money(b.pv), { x: c2[4], y: ty, size: 8, font: bold, color: slate })
    ty -= 14
  })
  ty -= 16
  p2.drawText(`Winner: ${model.winner.name} at ${compactMoney(model.winner.emvPv)} present-value expected net.`, {
    x: 36, y: ty, size: 11, font: bold, color: navy,
  })
  return doc.save()
}

export function downloadPdf(bytes: Uint8Array, filename: string) {
  const blob = new Blob([bytes.buffer as ArrayBuffer], { type: "application/pdf" })
  const url = URL.createObjectURL(blob)
  const a = document.createElement("a")
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}
