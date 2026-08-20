import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  evaluateCondition,
  isSupportedExpression,
  ExpressionError,
} from '../lib/workflow/expressions'

// ---------------------------------------------------------------------------
// Story 187 — no silent-false. Supported expressions evaluate deterministically;
// unsupported/malformed expressions raise an explicit error.
// ---------------------------------------------------------------------------

test('boolean true comparison evaluates true/false deterministically', () => {
  assert.equal(evaluateCondition('flag == true', { flag: true }), true)
  assert.equal(evaluateCondition('flag == true', { flag: false }), false)
  assert.equal(evaluateCondition('flag == false', { flag: false }), true)
  assert.equal(evaluateCondition('flag == false', { flag: true }), false)
})

test('null comparison: explicit null matches, missing fact does not collapse', () => {
  assert.equal(evaluateCondition('v == null', { v: null }), true)
  assert.equal(evaluateCondition('v == null', {}), false, 'missing must not equal null')
  assert.equal(evaluateCondition('v != null', { v: 'x' }), true)
})

test('string comparison', () => {
  assert.equal(evaluateCondition('role == "notario"', { role: 'notario' }), true)
  assert.equal(evaluateCondition("role == 'notario'", { role: 'title_company' }), false)
  assert.equal(evaluateCondition('role != "notario"', { role: 'attorney' }), true)
})

test('number comparison', () => {
  assert.equal(evaluateCondition('amount == 100000', { amount: 100000 }), true)
  assert.equal(evaluateCondition('amount != 100000', { amount: 99999 }), true)
})

test('missing fact is undefined and matches neither true nor null', () => {
  assert.equal(evaluateCondition('flag == true', {}), false)
  assert.equal(evaluateCondition('flag == null', {}), false)
})

test('malformed expressions raise an explicit error (never silent-false)', () => {
  for (const expr of [
    'flag =',           // missing literal/operator
    'flag',             // no operator
    'flag == true junk',// trailing garbage
    '== true',          // missing identifier
  ]) {
    assert.throws(() => evaluateCondition(expr, {}), ExpressionError, `expected throw for ${expr}`)
  }
})

test('unsupported expressions raise an explicit error', () => {
  for (const expr of [
    'a && b',
    'a || b',
    '!a',
    'fn()',
    'flag === true',
    'flag !== true',
    'amount > 100000',
    'a ==',
  ]) {
    assert.throws(() => evaluateCondition(expr, {}), ExpressionError, `expected throw for ${expr}`)
  }
})

test('isSupportedExpression accepts the supported DSL only', () => {
  assert.equal(isSupportedExpression('financingApplicable == true'), true)
  assert.equal(isSupportedExpression('financingApplicable == false'), true)
  assert.equal(isSupportedExpression('appraisalApplicable == null'), true)
  assert.equal(isSupportedExpression('requiresSurvey == true'), true)
  assert.equal(isSupportedExpression('closingAgentRole == "notario"'), true)
  assert.equal(isSupportedExpression("closingAgentRole != 'attorney'"), true)
  assert.equal(isSupportedExpression('amount == 100000'), true)
  assert.equal(isSupportedExpression('amount != 1.5'), true)

  assert.equal(isSupportedExpression('financingApplicable ='), false)
  assert.equal(isSupportedExpression('financingApplicable === true'), false)
  assert.equal(isSupportedExpression('foo && bar'), false)
  assert.equal(isSupportedExpression('arbitraryFunction()'), false)
  assert.equal(isSupportedExpression('flag == true junk'), false)
  assert.equal(isSupportedExpression('amount > 100000'), false)
  assert.equal(isSupportedExpression(''), false)
})
