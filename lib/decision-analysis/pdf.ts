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
const teal = rgb(0.059, 0.431, 0.42)
const clay = rgb(0.722, 0.361, 0.22)

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
  const bold = await doc.embedFont(StandardFonts.TimesBold)
  const italic = await doc.embedFont(StandardFonts.TimesItalic)

  const page = doc.addPage([612, 792])
  const W = 612
  const H = 792

  page.drawRectangle({ x: 0, y: H - 58, width: W, height: 58, color: navy })
  page.drawRectangle({ x: 0, y: H - 62, width: W, height: 4, color: gold })
  page.drawText("Executive summary  -  three-path exit", { x: 36, y: H - 32, size: 16, font: bold, color: white })
  page.drawText(inputs.propertyName, { x: 36, y: H - 48, size: 9, font: italic, color: rgb(0.84, 0.85, 0.86) })
  page.drawRectangle({ x: 0, y: 0, width: W, height: 28, color: navy })
  page.drawText("Decision analysis  -  not an appraisal or tax opinion", { x: 36, y: 11, size: 8, font, color: white })
  page.drawText("1 / 2", { x: W - 54, y: 11, size: 8, font, color: white })

  let y = H - 88
  page.drawRectangle({ x: 36, y: y - 48, width: W - 72, height: 52, color: rgb(0.918, 0.949, 0.973), borderColor: navy, borderWidth: 1 })
  page.drawText(`Recommendation:  ${model.winner.name}`, { x: 48, y: y - 16, size: 11, font: bold, color: navy })
  wrap(`Highest PV expected net (${money(model.winner.emvPv)}) in about ${model.winner.months} months. Sunk capital does not decide the next path.`, 92).forEach((line, i) => {
    page.drawText(line, { x: 48, y: y - 32 - i * 11, size: 9, font, color: slate })
  })
  y -= 78

  page.drawText("How it was scored", { x: 36, y, size: 12, font: bold, color: navy })
  y -= 14
  wrap("Squares are choices. Circles are chance. Each tip is sale price minus selling costs, remaining capex, plus salvage, minus placeholder tax. Tips are present-valued. Ranking metric is PV EMV.", 98).forEach((line) => {
    page.drawText(line, { x: 36, y, size: 10, font, color: slate })
    y -= 13
  })
  y -= 8

  const cardW = (W - 72 - 16) / 3
  model.scores.forEach((s, i) => {
    const x = 36 + i * (cardW + 8)
    const accent = i === 0 ? navy : i === 1 ? teal : clay
    page.drawRectangle({ x, y: y - 92, width: cardW, height: 100, color: cream, borderColor: accent, borderWidth: 1.2 })
    page.drawRectangle({ x, y: y - 8, width: cardW, height: 16, color: accent })
    page.drawText(s.name, { x: x + 8, y: y - 4, size: 8, font: bold, color: white })
    page.drawText(`${s.months} months`, { x: x + 8, y: y - 24, size: 9, font: bold, color: accent })
    page.drawText(`Future cash  ${money(s.futureCash)}`, { x: x + 8, y: y - 40, size: 8, font, color: slate })
    page.drawText(money(s.emvPv), { x: x + 8, y: y - 68, size: 13, font: bold, color: navy })
    page.drawText(s.best ? "BEST" : "PV expected net", { x: x + 8, y: y - 84, size: 8, font: italic, color: muted })
  })
  y -= 120

  page.drawText("Key inputs", { x: 36, y, size: 12, font: bold, color: navy })
  y -= 6
  const rows: [string, string][] = [
    ["As-is appraisal", money(inputs.appraisal)],
    ["Purchase + extra spent", `${money(inputs.purchasePrice)} + ${money(inputs.extraSpent)}`],
    ["Contributory of extra spend", money(inputs.contributoryToDate)],
    ["Selling / discount / tax", `${pct(inputs.sellingCostPct)} / ${pct(inputs.discountRate)} / ${pct(inputs.taxRate)}`],
    ["Option 1 odds", `${pct(inputs.o1PLow)} / ${pct(inputs.o1PMid)} / ${pct(inputs.o1PHigh)}`],
    ["Option 2 capex @ recovery", `${money(inputs.o2Capex)} @ ${pct(inputs.o2Recovery)}`],
    ["Option 3 launch + P(success)", `${money(inputs.o3Capex)} / ${pct(inputs.o3PSuccess)}`],
    ["NOI / cap rate", `${money(inputs.o3Noi)} / ${pct(inputs.o3CapRate)}`],
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

  y -= 28
  page.drawText("Disclaimer", { x: 36, y, size: 10, font: bold, color: navy })
  y -= 12
  wrap("Working model for a meeting, not a valuation, appraisal, or tax opinion.", 98).forEach((line) => {
    page.drawText(line, { x: 36, y, size: 8, font: italic, color: muted })
    y -= 11
  })

  const p2 = doc.addPage([792, 612])
  const W2 = 792
  const H2 = 612
  p2.drawRectangle({ x: 0, y: H2 - 52, width: W2, height: 52, color: navy })
  p2.drawRectangle({ x: 0, y: H2 - 56, width: W2, height: 4, color: gold })
  p2.drawText("Decision tree  -  probabilities, clocks, fold-back values", { x: 36, y: H2 - 28, size: 15, font: bold, color: white })
  p2.drawText("Squares = decisions    Circles = chance    Fold right to left", { x: 36, y: H2 - 44, size: 8, font: italic, color: rgb(0.84, 0.85, 0.86) })
  p2.drawRectangle({ x: 0, y: 0, width: W2, height: 24, color: navy })
  p2.drawText("2 / 2", { x: W2 - 48, y: 9, size: 8, font, color: white })

  const headers = ["Branch", "P", "Gross price", "After-tax net", "PV"]
  const cols = [36, 220, 280, 420, 560]
  let ty = H2 - 80
  p2.drawRectangle({ x: 36, y: ty - 4, width: W2 - 72, height: 16, color: navy })
  headers.forEach((h, i) => p2.drawText(h, { x: cols[i], y: ty, size: 8, font: bold, color: white }))
  ty -= 18
  model.branches.forEach((b, i) => {
    if (i % 2 === 0) p2.drawRectangle({ x: 36, y: ty - 3, width: W2 - 72, height: 14, color: cream })
    p2.drawText(b.label, { x: cols[0], y: ty, size: 8, font, color: slate })
    p2.drawText(pct(b.p), { x: cols[1], y: ty, size: 8, font, color: slate })
    p2.drawText(money(b.price), { x: cols[2], y: ty, size: 8, font, color: slate })
    p2.drawText(money(b.afterTax), { x: cols[3], y: ty, size: 8, font, color: slate })
    p2.drawText(money(b.pv), { x: cols[4], y: ty, size: 8, font: bold, color: slate })
    ty -= 14
  })

  ty -= 10
  p2.drawText("Fold-back", { x: 36, y: ty, size: 12, font: bold, color: navy })
  ty -= 16
  model.scores.forEach((s) => {
    p2.drawText(`${s.name}   ${s.months} mo   future ${money(s.futureCash)}   PV EMV ${money(s.emvPv)}   ${s.best ? "BEST" : ""}`, {
      x: 36,
      y: ty,
      size: 10,
      font: s.best ? bold : font,
      color: s.best ? navy : slate,
    })
    ty -= 16
  })
  ty -= 8
  p2.drawText(`Winner: ${model.winner.name} at ${compactMoney(model.winner.emvPv)} present-value expected net.`, {
    x: 36,
    y: ty,
    size: 11,
    font: bold,
    color: navy,
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
