// ---------------------------------------------------------------------------
// SELLER-STRATEGY INSIGHTS — deterministic, model-derived presentation logic
// for the Seller Strategy V2 cockpit.
//
// This is NOT decision mathematics. The single calculation engine remains
// lib/decision-analysis/model.evaluate(). Everything here only READS the
// produced ModelResult (+ the live Inputs) to derive factual observations:
// ranking, a "why this strategy" rationale, and key takeaways. No AI, no
// persistence, no new analytical model.
// ---------------------------------------------------------------------------

import { money, pct } from "../decision-analysis/model"
import type { Inputs, ModelResult, OptionScore } from "../decision-analysis/types"

/** Enabled strategies ranked highest-to-lowest by expected present value. */
export function rankStrategies(model: ModelResult): OptionScore[] {
  return model.scores.filter((s) => s.on).sort((a, b) => b.emvPv - a.emvPv)
}

/** Deterministic short explanation of why the current winner leads. */
export function buildRecommendationRationale(
  model: ModelResult,
  inputs: Inputs,
): string {
  const w = model.winner
  const enabled = model.scores.filter((s) => s.on)
  const shortestMonths = enabled.length ? Math.min(...enabled.map((s) => s.months)) : 0

  let strategy: string
  switch (w.option) {
    case 1:
      strategy = "Selling as-is requires no additional capital"
      break
    case 2:
      strategy = "The invested improvements are expected to lift proceeds above the as-is path"
      break
    case 3:
      strategy = "The stabilized package is expected to maximize value across the combined property + business interest"
      break
    case 4:
      strategy = "Separating the property and the other interests is expected to release more value than selling together"
      break
    case 5:
      strategy = "Holding or selling only a share is expected to preserve optionality while still returning value"
      break
    default:
      strategy = "This path leads on expected present value"
  }

  const parts: string[] = [
    `${w.short} currently provides the highest expected present value of ${money(w.emvPv)}`,
  ]
  if (w.months <= shortestMonths) {
    parts.push("with the shortest path to liquidity")
  }
  if (w.futureCash > 0) {
    parts.push(`requiring ${money(w.futureCash)} of incremental capital`)
  }
  const gap = model.runnerUp ? w.emvPv - model.runnerUp.emvPv : 0
  if (model.runnerUp && gap > 0) {
    parts.push(`beating ${model.runnerUp.short} by ${money(gap)}`)
  }
  return `${strategy} — ${parts.join(", ")}.`
}

export type Takeaway = {
  tone: "positive" | "neutral" | "caution"
  text: string
}

/** Factual, traceable observations derived from the displayed model. */
export function buildTakeaways(model: ModelResult, inputs: Inputs): Takeaway[] {
  const out: Takeaway[] = []
  const enabled = model.scores.filter((s) => s.on)

  // Highest expected PV.
  if (enabled.length) {
    out.push({
      tone: "positive",
      text: `${model.winner.short} carries the highest expected PV at ${money(model.winner.emvPv)}.`,
    })
  }

  // Shortest liquidity horizon.
  if (enabled.length) {
    const fastest = [...enabled].sort((a, b) => a.months - b.months)[0]
    if (fastest.option !== model.winner.option) {
      out.push({
        tone: "neutral",
        text: `${fastest.short} reaches liquidity soonest at ${fastest.months} months.`,
      })
    }
  }

  // Largest incremental capital requirement.
  if (enabled.length) {
    const heaviest = [...enabled].sort((a, b) => b.futureCash - a.futureCash)[0]
    if (heaviest.futureCash > 0) {
      out.push({
        tone: "caution",
        text: `${heaviest.short} requires the most incremental capital at ${money(heaviest.futureCash)}.`,
      })
    }
  }

  // Longest execution duration.
  if (enabled.length) {
    const longest = [...enabled].sort((a, b) => b.months - a.months)[0]
    out.push({
      tone: "neutral",
      text: `${longest.short} has the longest horizon at ${longest.months} months.`,
    })
  }

  // Downside / failure exposure.
  if (model.branches.length) {
    const worst = [...model.branches].sort((a, b) => a.pv - b.pv)[0]
    out.push({
      tone: "caution",
      text: `The lowest single outcome is ${worst.label} at ${money(worst.pv)} PV.`,
    })
  }
  if (inputs.o3On && inputs.o3PSuccess < 1) {
    out.push({
      tone: "caution",
      text: `Join carries a ${pct(1 - inputs.o3PSuccess)} failure branch at ${money(inputs.o3FailSalvage)} salvage.`,
    })
  }

  // Model validity flags, if any.
  if (model.flags.length) {
    for (const f of model.flags.slice(0, 2)) {
      out.push({ tone: "caution", text: f })
    }
  }

  return out
}
