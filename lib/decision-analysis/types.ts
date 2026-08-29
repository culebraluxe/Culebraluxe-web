export type OptionId = 1 | 2 | 3 | 4 | 5

export type Inputs = {
  propertyName: string
  purchasePrice: number
  extraSpent: number
  contributoryToDate: number
  appraisal: number
  sellingCostPct: number
  discountRate: number
  taxRate: number

  o1On: boolean
  o1Months: number
  o1Salvage: number
  o1LowDelta: number
  o1MidDelta: number
  o1HighDelta: number
  o1PLow: number
  o1PMid: number
  o1PHigh: number

  o2On: boolean
  o2Capex: number
  o2Months: number
  o2Recovery: number
  o2Salvage: number
  o2LowDelta: number
  o2MidDelta: number
  o2HighDelta: number
  o2PLow: number
  o2PMid: number
  o2PHigh: number

  o3On: boolean
  o3Capex: number
  o3Months: number
  o3Noi: number
  o3CapRate: number
  o3PSuccess: number
  o3FailSalvage: number
  o3LowDelta: number
  o3MidDelta: number
  o3HighDelta: number
  o3PLow: number
  o3PMid: number
  o3PHigh: number

  o4On: boolean
  o4Capex: number
  o4Months: number
  o4AssetBase: number
  o4Salvage: number
  o4LowDelta: number
  o4MidDelta: number
  o4HighDelta: number
  o4PLow: number
  o4PMid: number
  o4PHigh: number

  o5On: boolean
  o5Share: number
  o5Months: number
  o5PeriodCash: number
  o5Capex: number
  o5LowDelta: number
  o5HighDelta: number
  o5MidDelta: number
  o5PLow: number
  o5PMid: number
  o5PHigh: number
}

export type Branch = {
  id: string
  label: string
  option: OptionId
  price: number
  p: number
  sellingCosts: number
  futureCapex: number
  salvage: number
  pretaxNet: number
  tax: number
  afterTax: number
  pv: number
}

export type OptionScore = {
  option: OptionId
  name: string
  short: string
  on: boolean
  months: number
  futureCash: number
  emvUndiscounted: number
  emvPv: number
  vsOption1: number
  best: boolean
}

export type ModelResult = {
  basis: number
  branches: Branch[]
  scores: OptionScore[]
  winner: OptionScore
  runnerUp?: OptionScore
  flags: string[]
}
