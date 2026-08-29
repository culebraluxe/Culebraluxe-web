import test from 'node:test'
import assert from 'node:assert/strict'

import { DEFAULT_INPUTS } from '../../lib/decision-analysis/defaults'
import { evaluate, money } from '../../lib/decision-analysis/model'
import {
  buildRecommendationRationale,
  buildTakeaways,
  rankStrategies,
} from '../../lib/seller-strategy/insights'
import type { OptionId } from '../../lib/decision-analysis/types'

test('SELLER-STRATEGY 1: ranking returns only enabled strategies, highest PV first = winner', () => {
  const model = evaluate(DEFAULT_INPUTS)
  const ranking = rankStrategies(model)
  assert.ok(ranking.length >= 1)
  assert.equal(ranking[0].option, model.winner.option)
  for (let i = 1; i < ranking.length; i++) {
    assert.ok(ranking[i - 1].emvPv >= ranking[i].emvPv, 'ranked descending by emvPv')
  }
})

test('SELLER-STRATEGY 2: disabling a path excludes it from ranking (model semantics)', () => {
  const disabled = evaluate({ ...DEFAULT_INPUTS, o2On: false })
  const ranking = rankStrategies(disabled)
  assert.ok(ranking.every((s) => s.option !== 2), 'option 2 excluded when off')
  assert.ok(disabled.scores.find((s) => s.option === 2)?.on === false)
})

test('SELLER-STRATEGY 3: recommendation rationale is deterministic and references live values', () => {
  const model = evaluate(DEFAULT_INPUTS)
  const rationale = buildRecommendationRationale(model, DEFAULT_INPUTS)
  assert.ok(typeof rationale === 'string' && rationale.length > 0)
  assert.ok(rationale.includes(model.winner.short), 'mentions winner')
  assert.ok(rationale.includes(money(model.winner.emvPv)), 'mentions winner expected PV')
})

test('SELLER-STRATEGY 4: the recommendation can change when assumptions change', () => {
  const baseModel = evaluate(DEFAULT_INPUTS)
  const baseWinner = rankStrategies(baseModel)[0].option

  // Weaken whichever strategy currently wins; a different path must lead.
  let alt: typeof DEFAULT_INPUTS = { ...DEFAULT_INPUTS }
  switch (baseWinner) {
    case 3:
      alt = { ...DEFAULT_INPUTS, o3Noi: 0 } // Join package value collapses
      break
    case 1:
      alt = { ...DEFAULT_INPUTS, o1LowDelta: -0.9, o1MidDelta: -0.9, o1HighDelta: -0.9 }
      break
    case 2:
      alt = { ...DEFAULT_INPUTS, o2Capex: 0, o2Recovery: 0 }
      break
    case 4:
      alt = { ...DEFAULT_INPUTS, o4AssetBase: 0 }
      break
    case 5:
      alt = { ...DEFAULT_INPUTS, o5Share: 0.1 }
      break
  }
  const altWinner = rankStrategies(evaluate(alt))[0].option
  assert.notEqual(altWinner, baseWinner, 'weakening the leader must change the recommendation')
})

test('SELLER-STRATEGY 5: takeaways are deterministic, non-empty, and traceable to live values', () => {
  const model = evaluate(DEFAULT_INPUTS)
  const takeaways = buildTakeaways(model, DEFAULT_INPUTS)
  assert.ok(takeaways.length > 0)
  for (const t of takeaways) {
    assert.ok(t.text.length > 0)
    assert.ok(['positive', 'neutral', 'caution'].includes(t.tone), 'tone is valid')
  }
  assert.ok(
    takeaways.some((t) => t.text.includes(model.winner.short)),
    'a takeaway references the winner',
  )
})

test('SELLER-STRATEGY 6: changing a probability recalculates expected PV', () => {
  const base = evaluate(DEFAULT_INPUTS)
  // Shift more probability to the high as-is outcome.
  const shifted = evaluate({ ...DEFAULT_INPUTS, o1PLow: 0.2, o1PMid: 0.3, o1PHigh: 0.5 })
  const asIsBase = base.scores.find((s) => s.option === 1)?.emvPv ?? 0
  const asIsShifted = shifted.scores.find((s) => s.option === 1)?.emvPv ?? 0
  assert.notEqual(asIsShifted, asIsBase, 'probability change alters expected PV')
})

test('SELLER-STRATEGY 7: changing discount rate changes PV results', () => {
  const base = evaluate(DEFAULT_INPUTS)
  const slower = evaluate({ ...DEFAULT_INPUTS, discountRate: 0.03 })
  const opt: OptionId = 1
  const b = base.scores.find((s) => s.option === opt)?.emvPv ?? 0
  const s = slower.scores.find((s) => s.option === opt)?.emvPv ?? 0
  assert.ok(s > b, 'lower discount rate raises present value')
})
