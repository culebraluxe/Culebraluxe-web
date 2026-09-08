import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  DEFAULT_UNIT_RATES,
  costWidgets,
  estimateWork,
  estimatedRisk,
  modelWidgetWeight,
  ratesFromActuals,
  workPoints,
  type WorkEstimateFactors,
} from '../forge/forge-estimator'

function factors(over: Partial<WorkEstimateFactors>): WorkEstimateFactors {
  return {
    workstream: 'ENG',
    complexity: 'medium',
    toolkit: 'brownfield',
    modelGrade: 'flash',
    seamsCount: 3,
    acceptanceCount: 5,
    ...over,
  }
}

test('estimator: points rise with complexity, greenfield, seams and acceptance', () => {
  const low = workPoints(factors({ complexity: 'low', toolkit: 'brownfield', seamsCount: 1, acceptanceCount: 1 }))
  const high = workPoints(factors({ complexity: 'high', toolkit: 'greenfield', seamsCount: 8, acceptanceCount: 12 }))
  assert.ok(high > low)
  assert.ok(workPoints(factors({ toolkit: 'greenfield' })) > workPoints(factors({ toolkit: 'brownfield' })))
})

test('estimator: pro costs far more per run but is faster per point (the tradeoff is exposed)', () => {
  const base = factors({ modelGrade: 'flash' })
  const flash = estimateWork(base)
  const pro = estimateWork({ ...base, modelGrade: 'pro' })
  assert.equal(flash.points, pro.points) // same work, same points
  assert.ok(pro.estimatedWidgets > flash.estimatedWidgets)
  assert.ok(pro.estimatedMinutes < flash.estimatedMinutes)
})

test('estimator: risk flags greenfield+medium and high complexity', () => {
  assert.equal(estimatedRisk(factors({ complexity: 'low', toolkit: 'brownfield', seamsCount: 1, acceptanceCount: 1 })), 'low')
  assert.equal(estimatedRisk(factors({ complexity: 'medium', toolkit: 'greenfield' })), 'high')
  assert.equal(estimatedRisk(factors({ complexity: 'high' })), 'high')
})

test('estimator: recalibrating from real actuals corrects the unit cost rate', () => {
  const actuals = [
    { points: 10, tokens: 20_000, costWidgets: 1.5, minutes: 30 },
    { points: 10, tokens: 24_000, costWidgets: 2.1, minutes: 36 },
  ]
  const r = ratesFromActuals(actuals)
  assert.ok(r.widgetsPerPoint > DEFAULT_UNIT_RATES.flash.widgetsPerPoint)
  const est = estimateWork(factors({}), r)
  assert.ok(est.estimatedWidgets > estimateWork(factors({})).estimatedWidgets)
  // Empty actuals fall back to defaults.
  assert.equal(ratesFromActuals([]).widgetsPerPoint, DEFAULT_UNIT_RATES.flash.widgetsPerPoint)
})

test('cost widgets: model weight x elapsed minutes; assay is free; unknown is null', () => {
  assert.equal(modelWidgetWeight('deepseek/deepseek-v4-flash'), 1)
  assert.equal(modelWidgetWeight('deepseek/deepseek-chat'), 4)
  assert.equal(modelWidgetWeight('forge/deterministic-assay'), 0)
  assert.equal(costWidgets('deepseek/deepseek-v4-flash', 30), 30) // 1 x 30m
  assert.equal(costWidgets('forge/deterministic-assay', 30), 0)
  assert.equal(costWidgets('some-unknown-model', 30), null)
  assert.equal(costWidgets(null, 30), null)
})
