import type { Branch, Inputs, ModelResult, OptionId, OptionScore } from "./types"

const near = (a: number, b: number, eps = 0.001) => Math.abs(a - b) < eps

function pv(amount: number, rate: number, months: number) {
  return amount / Math.pow(1 + rate, months / 12)
}

function branch(
  partial: Omit<Branch, "sellingCosts" | "pretaxNet" | "tax" | "afterTax" | "pv">,
  inputs: Inputs,
  months: number,
  basis: number,
): Branch {
  const sellingCosts = partial.price * inputs.sellingCostPct
  const pretaxNet = partial.price - sellingCosts - partial.futureCapex + partial.salvage
  const tax = Math.max(0, partial.price - basis) * inputs.taxRate
  const afterTax = pretaxNet - tax
  return {
    ...partial,
    sellingCosts,
    pretaxNet,
    tax,
    afterTax,
    pv: pv(afterTax, inputs.discountRate, months),
  }
}

export function evaluate(inputs: Inputs): ModelResult {
  const basis = inputs.purchasePrice + inputs.contributoryToDate
  const flags: string[] = []

  const checkTriple = (sum: number, label: string, on: boolean) => {
    if (on && !near(sum, 1)) flags.push(`${label} probabilities must sum to 100%.`)
  }
  checkTriple(inputs.o1PLow + inputs.o1PMid + inputs.o1PHigh, "As-is", inputs.o1On)
  checkTriple(inputs.o2PLow + inputs.o2PMid + inputs.o2PHigh, "Improve", inputs.o2On)
  checkTriple(inputs.o3PLow + inputs.o3PMid + inputs.o3PHigh, "Join", inputs.o3On)
  checkTriple(inputs.o4PLow + inputs.o4PMid + inputs.o4PHigh, "Fork", inputs.o4On)
  checkTriple(inputs.o5PLow + inputs.o5PMid + inputs.o5PHigh, "Hold", inputs.o5On)
  if (inputs.o3On && inputs.o3CapRate <= 0) flags.push("Join cap rate must be greater than 0.")
  if (inputs.o3On && (inputs.o3PSuccess < 0 || inputs.o3PSuccess > 1)) flags.push("Join P(success) must be 0-100%.")
  if (inputs.o5On && (inputs.o5Share <= 0 || inputs.o5Share > 1)) flags.push("Hold/recap share must be between 0 and 100%.")
  if (![inputs.o1On, inputs.o2On, inputs.o3On, inputs.o4On, inputs.o5On].some(Boolean)) {
    flags.push("Turn on at least one path.")
  }

  const o1 = {
    low: inputs.appraisal * (1 + inputs.o1LowDelta),
    mid: inputs.appraisal * (1 + inputs.o1MidDelta),
    high: inputs.appraisal * (1 + inputs.o1HighDelta),
  }
  const improved = inputs.appraisal + inputs.o2Capex * inputs.o2Recovery
  const o2 = {
    low: improved * (1 + inputs.o2LowDelta),
    mid: improved * (1 + inputs.o2MidDelta),
    high: improved * (1 + inputs.o2HighDelta),
  }
  const pack = inputs.o3CapRate > 0 ? inputs.o3Noi / inputs.o3CapRate : 0
  const o3 = {
    low: pack * (1 + inputs.o3LowDelta),
    mid: pack * (1 + inputs.o3MidDelta),
    high: pack * (1 + inputs.o3HighDelta),
  }
  const o4 = {
    low: inputs.appraisal * (1 + inputs.o4LowDelta) + inputs.o4AssetBase * (1 + inputs.o4LowDelta),
    mid: inputs.appraisal * (1 + inputs.o4MidDelta) + inputs.o4AssetBase * (1 + inputs.o4MidDelta),
    high: inputs.appraisal * (1 + inputs.o4HighDelta) + inputs.o4AssetBase * (1 + inputs.o4HighDelta),
  }
  const share = Math.min(1, Math.max(0, inputs.o5Share))
  const o5 = {
    low: inputs.appraisal * (1 + inputs.o5LowDelta) * share,
    mid: inputs.appraisal * (1 + inputs.o5MidDelta) * share,
    high: inputs.appraisal * (1 + inputs.o5HighDelta) * share,
  }

  const branches: Branch[] = []
  const push = (b: Branch, on: boolean) => {
    if (on) branches.push(b)
  }

  push(branch({ id: "1L", label: "1-Low", option: 1, price: o1.low, p: inputs.o1PLow, futureCapex: 0, salvage: inputs.o1Salvage }, inputs, inputs.o1Months, basis), inputs.o1On)
  push(branch({ id: "1M", label: "1-Mid", option: 1, price: o1.mid, p: inputs.o1PMid, futureCapex: 0, salvage: inputs.o1Salvage }, inputs, inputs.o1Months, basis), inputs.o1On)
  push(branch({ id: "1H", label: "1-Ideal", option: 1, price: o1.high, p: inputs.o1PHigh, futureCapex: 0, salvage: inputs.o1Salvage }, inputs, inputs.o1Months, basis), inputs.o1On)

  push(branch({ id: "2L", label: "2-Low", option: 2, price: o2.low, p: inputs.o2PLow, futureCapex: inputs.o2Capex, salvage: inputs.o2Salvage }, inputs, inputs.o2Months, basis), inputs.o2On)
  push(branch({ id: "2M", label: "2-Base", option: 2, price: o2.mid, p: inputs.o2PMid, futureCapex: inputs.o2Capex, salvage: inputs.o2Salvage }, inputs, inputs.o2Months, basis), inputs.o2On)
  push(branch({ id: "2H", label: "2-High", option: 2, price: o2.high, p: inputs.o2PHigh, futureCapex: inputs.o2Capex, salvage: inputs.o2Salvage }, inputs, inputs.o2Months, basis), inputs.o2On)

  push(branch({ id: "3L", label: "3-Success Low", option: 3, price: o3.low, p: inputs.o3PSuccess * inputs.o3PLow, futureCapex: inputs.o3Capex, salvage: 0 }, inputs, inputs.o3Months, basis), inputs.o3On)
  push(branch({ id: "3M", label: "3-Success Base", option: 3, price: o3.mid, p: inputs.o3PSuccess * inputs.o3PMid, futureCapex: inputs.o3Capex, salvage: 0 }, inputs, inputs.o3Months, basis), inputs.o3On)
  push(branch({ id: "3H", label: "3-Success High", option: 3, price: o3.high, p: inputs.o3PSuccess * inputs.o3PHigh, futureCapex: inputs.o3Capex, salvage: 0 }, inputs, inputs.o3Months, basis), inputs.o3On)
  push(branch({ id: "3F", label: "3-Fail", option: 3, price: inputs.o3FailSalvage, p: 1 - inputs.o3PSuccess, futureCapex: inputs.o3Capex, salvage: 0 }, inputs, inputs.o3Months, basis), inputs.o3On)

  push(branch({ id: "4L", label: "4-Low house+assets", option: 4, price: o4.low, p: inputs.o4PLow, futureCapex: inputs.o4Capex, salvage: inputs.o4Salvage }, inputs, inputs.o4Months, basis), inputs.o4On)
  push(branch({ id: "4M", label: "4-Mid house+assets", option: 4, price: o4.mid, p: inputs.o4PMid, futureCapex: inputs.o4Capex, salvage: inputs.o4Salvage }, inputs, inputs.o4Months, basis), inputs.o4On)
  push(branch({ id: "4H", label: "4-High house+assets", option: 4, price: o4.high, p: inputs.o4PHigh, futureCapex: inputs.o4Capex, salvage: inputs.o4Salvage }, inputs, inputs.o4Months, basis), inputs.o4On)

  push(branch({ id: "5L", label: "5-Low terminal", option: 5, price: o5.low, p: inputs.o5PLow, futureCapex: inputs.o5Capex, salvage: inputs.o5PeriodCash }, inputs, inputs.o5Months, basis), inputs.o5On)
  push(branch({ id: "5M", label: "5-Mid terminal", option: 5, price: o5.mid, p: inputs.o5PMid, futureCapex: inputs.o5Capex, salvage: inputs.o5PeriodCash }, inputs, inputs.o5Months, basis), inputs.o5On)
  push(branch({ id: "5H", label: "5-High terminal", option: 5, price: o5.high, p: inputs.o5PHigh, futureCapex: inputs.o5Capex, salvage: inputs.o5PeriodCash }, inputs, inputs.o5Months, basis), inputs.o5On)

  const emv = (opt: OptionId, field: "pv" | "afterTax") =>
    branches.filter((b) => b.option === opt).reduce((s, b) => s + b.p * b[field], 0)

  const catalog: { option: OptionId; name: string; short: string; on: boolean; months: number; futureCash: number }[] = [
    { option: 1, name: "1  Sell as-is", short: "As-is", on: inputs.o1On, months: inputs.o1Months, futureCash: 0 },
    { option: 2, name: "2  Improve then sell", short: "Improve", on: inputs.o2On, months: inputs.o2Months, futureCash: inputs.o2Capex },
    { option: 3, name: "3  Join then sell package", short: "Join", on: inputs.o3On, months: inputs.o3Months, futureCash: inputs.o3Capex },
    { option: 4, name: "4  Fork house and assets", short: "Fork", on: inputs.o4On, months: inputs.o4Months, futureCash: inputs.o4Capex },
    { option: 5, name: "5  Hold or recap", short: "Hold", on: inputs.o5On, months: inputs.o5Months, futureCash: inputs.o5Capex - inputs.o5PeriodCash },
  ]

  const raw = catalog.map((c) => ({
    ...c,
    emvUndiscounted: c.on ? emv(c.option, "afterTax") : Number.NEGATIVE_INFINITY,
    emvPv: c.on ? emv(c.option, "pv") : Number.NEGATIVE_INFINITY,
  }))
  const active = raw.filter((s) => s.on)
  const bestPv = active.length ? Math.max(...active.map((s) => s.emvPv)) : Number.NEGATIVE_INFINITY
  const o1Pv = raw[0].on ? raw[0].emvPv : 0
  const scores: OptionScore[] = raw.map((s) => ({
    ...s,
    emvUndiscounted: s.on ? s.emvUndiscounted : 0,
    emvPv: s.on ? s.emvPv : 0,
    vsOption1: s.on && raw[0].on ? s.emvPv - o1Pv : 0,
    best: s.on && s.emvPv === bestPv,
  }))
  const ranked = scores.filter((s) => s.on).sort((a, b) => b.emvPv - a.emvPv)
  return {
    basis,
    branches,
    scores,
    winner: ranked[0] ?? scores[0],
    runnerUp: ranked[1],
    flags,
  }
}

export function money(n: number) {
  const abs = Math.abs(Math.round(n))
  const formatted = abs.toLocaleString("en-US")
  return n < 0 ? `($${formatted})` : `$${formatted}`
}

export function pct(n: number) {
  return `${(n * 100).toFixed(1)}%`
}

export function compactMoney(n: number) {
  const k = n / 1000
  const sign = k < 0 ? "-" : ""
  return `${sign}$${Math.abs(k).toFixed(0)}k`
}
