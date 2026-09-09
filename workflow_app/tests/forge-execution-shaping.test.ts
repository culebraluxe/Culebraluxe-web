import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  type SmithChunk,
  type SmithExecutionPlan,
  validateSmithExecutionPlan,
} from '../forge/forge-execution-shaping'

const chunk = (id: number): SmithChunk => ({
  id,
  outcome: `outcome ${id}`,
  surface: [`db/x.ts`],
  invariant: `invariant ${id}`,
  proof: `targeted test for ${id}`,
})

const plan = (chunks: SmithChunk[], size: SmithExecutionPlan['size'] = 'MEDIUM'): SmithExecutionPlan => ({
  size,
  chunks,
})

test('execution-shaping: a valid 2-chunk plan passes the hard structural rules', () => {
  assert.deepEqual(validateSmithExecutionPlan(plan([chunk(1), chunk(2)])), [])
})

test('execution-shaping: a single-chunk SMALL plan is structurally sound', () => {
  assert.deepEqual(validateSmithExecutionPlan(plan([chunk(1)], 'SMALL')), [])
})

test('execution-shaping: a 4th chunk is a HOLD, not keep working', () => {
  const violations = validateSmithExecutionPlan(plan([chunk(1), chunk(2), chunk(3), chunk(4)]))
  assert.ok(violations.some((v) => v.includes('a 4th chunk is HOLD')))
})

test('execution-shaping: a chunk missing outcome/surface/proof/invariant fails', () => {
  const bad: SmithChunk = { id: 1, outcome: '  ', surface: [], invariant: '', proof: '' }
  const violations = validateSmithExecutionPlan(plan([bad]))
  assert.ok(violations.some((v) => v.includes('missing one outcome')))
  assert.ok(violations.some((v) => v.includes('missing code surface')))
  assert.ok(violations.some((v) => v.includes('missing invariant')))
  assert.ok(violations.some((v) => v.includes('missing runnable targeted proof')))
})

test('execution-shaping: missing plan and bad ids/deps are rejected', () => {
  assert.deepEqual(validateSmithExecutionPlan(null), ['missing smith_execution_plan'])
  const nonSerial = validateSmithExecutionPlan(plan([chunk(2), chunk(3)]))
  assert.ok(nonSerial.some((v) => v.includes('serial')))
})
