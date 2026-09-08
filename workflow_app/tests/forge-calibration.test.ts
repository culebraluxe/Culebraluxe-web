import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  MIN_FIT_SAMPLES,
  estimateConfidenceFactor,
  fitCalibratedModel,
  predictWithInterval,
  type CalibrationSample,
} from '../forge/forge-calibration'

// V3 stub contract: NOT active until real actuals exist. The estimate formula
// multiplies by an IDENTITY factor (1.0) so the path is unchanged until V3 is on.

function sample(i: number): CalibrationSample {
  return {
    storyId: `S${i}`,
    features: { points: 10, surfaceScore: 20, seamsCount: 3, complexityTier: 'medium', toolkit: 'brownfield', modelGrade: 'flash' },
    actual: { costWidgets: 15, minutes: 30, sloc: 200 },
    finishedAt: `2026-09-0${i}T00:00:00Z`,
  }
}

test('V3 calibration: identity confidence factor leaves the estimate path unchanged', () => {
  assert.equal(estimateConfidenceFactor(), 1.0)
})

test('V3 calibration: no learned model until implemented with real data (returns null)', () => {
  assert.equal(MIN_FIT_SAMPLES, 10)
  assert.equal(fitCalibratedModel(Array.from({ length: 20 }, (_, i) => sample(i))), null)
  assert.equal(predictWithInterval({ segments: [], rSquared: null, residualBias: null, sampleCount: 0 }, sample(0).features), null)
})
