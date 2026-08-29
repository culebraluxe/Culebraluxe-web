import type { Branch, Inputs, ModelResult, OptionScore } from "./types"

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

  const o1Sum = inputs.o1PLow + inputs.o1PMid + inputs.o1PHigh
  const o2Sum = inputs.o2PLow + inputs.o2PMid + inputs.o2PHigh
  const o3Sum = inputs.o3PLow + inputs.o3PMid + inputs.o3PHigh
  if (!near(o1Sum, 1)) flags.push("Option 1 probabilities must sum to 100%.")
  if (!near(o2Sum, 1)) flags.push("Option 2 probabilities must sum to 100%.")
  if (!near(o3Sum, 1)) flags.push("Option 3 conditional probabilities must sum to 100%.")
  if (inputs.o3CapRate <= 0) flags.push("Cap rate must be greater than 0.")
  if (inputs.o3PSuccess < 0 || inputs.o3PSuccess > 1) flags.push("P(success) must be between 0 and 100%.")

  const o1Prices = {
    low: inputs.appraisal * (1 + inputs.o1LowDelta),
    mid: inputs.appraisal * (1 + inputs.o1MidDelta),
    high: inputs.appraisal * (1 + inputs.o1HighDelta),
  }
  const improvedBase = inputs.appraisal + inputs.o2Capex * inputs.o2Recovery
  const o2Prices = {
    low: improvedBase * (1 + inputs.o2LowDelta),
    mid: improvedBase * (1 + inputs.o2MidDelta),
    high: improvedBase * (1 + inputs.o2HighDelta),
  }
  const packageBase = inputs.o3CapRate > 0 ? inputs.o3Noi / inputs.o3CapRate : 0
  const o3Prices = {
    low: packageBase * (1 + inputs.o3LowDelta),
    mid: packageBase * (1 + inputs.o3MidDelta),
    high: packageBase * (1 + inputs.o3HighDelta),
  }

  const branches: Branch[] = [
    branch(
      { id: "1L", label: "1-Low at appraisal", option: 1, price: o1Prices.low, p: inputs.o1PLow, futureCapex: 0, salvage: inputs.o1Salvage },
      inputs, inputs.o1Months, basis,
    ),
    branch(
      { id: "1M", label: "1-Mid", option: 1, price: o1Prices.mid, p: inputs.o1PMid, futureCapex: 0, salvage: inputs.o1Salvage },
      inputs, inputs.o1Months, basis,
    ),
    branch(
      { id: "1H", label: "1-Ideal", option: 1, price: o1Prices.high, p: inputs.o1PHigh, futureCapex: 0, salvage: inputs.o1Salvage },
      inputs, inputs.o1Months, basis,
    ),
    branch(
      { id: "2L", label: "2-Low", option: 2, price: o2Prices.low, p: inputs.o2PLow, futureCapex: inputs.o2Capex, salvage: inputs.o2Salvage },
      inputs, inputs.o2Months, basis,
    ),
    branch(
      { id: "2M", label: "2-Base", option: 2, price: o2Prices.mid, p: inputs.o2PMid, futureCapex: inputs.o2Capex, salvage: inputs.o2Salvage },
      inputs, inputs.o2Months, basis,
    ),
    branch(
      { id: "2H", label: "2-High", option: 2, price: o2Prices.high, p: inputs.o2PHigh, futureCapex: inputs.o2Capex, salvage: inputs.o2Salvage },
      inputs, inputs.o2Months, basis,
    ),
    branch(
      { id: "3L", label: "3-Success Low", option: 3, price: o3Prices.low, p: inputs.o3PSuccess * inputs.o3PLow, futureCapex: inputs.o3Capex, salvage: 0 },
      inputs, inputs.o3Months, basis,
    ),
    branch(
      { id: "3M", label: "3-Success Base", option: 3, price: o3Prices.mid, p: inputs.o3PSuccess * inputs.o3PMid, futureCapex: inputs.o3Capex, salvage: 0 },
      inputs, inputs.o3Months, basis,
    ),
    branch(
      { id: "3H", label: "3-Success High", option: 3, price: o3Prices.high, p: inputs.o3PSuccess * inputs.o3PHigh, futureCapex: inputs.o3Capex, salvage: 0 },
      inputs, inputs.o3Months, basis,
    ),
    branch(
      { id: "3F", label: "3-Fail", option: 3, price: inputs.o3FailSalvage, p: 1 - inputs.o3PSuccess, futureCapex: inputs.o3Capex, salvage: 0 },
      inputs, inputs.o3Months, basis,
    ),
  ]

  const emv = (opt: 1 | 2 | 3, field: "pv" | "afterTax") =>
    branches.filter((b) => b.option === opt).reduce((s, b) => s + b.p * b[field], 0)

  const raw: Omit<OptionScore, "best" | "vsOption1">[] = [
    { option: 1, name: "1  Sell as-is", months: inputs.o1Months, futureCash: 0, emvUndiscounted: emv(1, "afterTax"), emvPv: emv(1, "pv") },
    { option: 2, name: "2  Improve then sell", months: inputs.o2Months, futureCash: inputs.o2Capex, emvUndiscounted: emv(2, "afterTax"), emvPv: emv(2, "pv") },
    { option: 3, name: "3  Launch then sell package", months: inputs.o3Months, futureCash: inputs.o3Capex, emvUndiscounted: emv(3, "afterTax"), emvPv: emv(3, "pv") },
  ]
  const bestPv = Math.max(...raw.map((s) => s.emvPv))
  const scores: OptionScore[] = raw.map((s) => ({
    ...s,
    vsOption1: s.emvPv - raw[0].emvPv,
    best: s.emvPv === bestPv,
  }))

  return { basis, branches, scores, winner: scores.find((s) => s.best) ?? scores[0], flags }
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
