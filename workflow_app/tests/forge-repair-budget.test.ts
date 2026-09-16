import assert from 'node:assert/strict'
import test from 'node:test'
import {
  forgeRepairBudgetBlock,
  forgeRepairBudgetFromEvidence,
  type ForgeVisibilitySnapshot,
} from '../forge/forge-visibility'

test('a known cap reports the used count and what remains', () => {
  const block = forgeRepairBudgetBlock({
    repairAttempts: 1,
    replanAttempts: 0,
    maxRepairAttempts: 3,
    maxReplanAttempts: 2,
  })

  assert.deepEqual(block.repair, { used: 1, cap: 3, remaining: 2, exhausted: false })
  assert.deepEqual(block.replan, { used: 0, cap: 2, remaining: 2, exhausted: false })
})

test('a budget at its cap is exhausted with zero remaining', () => {
  const block = forgeRepairBudgetBlock({
    repairAttempts: 3,
    replanAttempts: 2,
    maxRepairAttempts: 3,
    maxReplanAttempts: 2,
  })

  assert.deepEqual(block.repair, { used: 3, cap: 3, remaining: 0, exhausted: true })
  assert.deepEqual(block.replan, { used: 2, cap: 2, remaining: 0, exhausted: true })
})

test('a budget past its cap never reports negative remaining', () => {
  const block = forgeRepairBudgetBlock({
    repairAttempts: 5,
    replanAttempts: 0,
    maxRepairAttempts: 3,
    maxReplanAttempts: 2,
  })

  assert.equal(block.repair.remaining, 0)
  assert.equal(block.repair.exhausted, true)
})

test('an unknown cap reports remaining as null, NEVER as zero', () => {
  const block = forgeRepairBudgetBlock({
    repairAttempts: 2,
    replanAttempts: 1,
    maxRepairAttempts: null,
    maxReplanAttempts: undefined,
  })

  assert.equal(block.repair.used, 2)
  assert.equal(block.repair.cap, null)
  assert.equal(block.repair.remaining, null)
  assert.equal(block.repair.exhausted, null)
  assert.notEqual(block.repair.remaining, 0)

  assert.equal(block.replan.cap, null)
  assert.equal(block.replan.remaining, null)
  assert.equal(block.replan.exhausted, null)
})

test('an absent durable counter reads null, never a fabricated zero', () => {
  const block = forgeRepairBudgetBlock({
    maxRepairAttempts: 3,
    maxReplanAttempts: 2,
  })

  assert.equal(block.repair.used, null)
  assert.equal(block.repair.remaining, null)
  assert.notEqual(block.repair.used, 0)

  assert.equal(block.replan.used, null)
  assert.equal(block.replan.remaining, null)
})

test('a measured zero used stays zero, not null', () => {
  const block = forgeRepairBudgetBlock({
    repairAttempts: 0,
    replanAttempts: 0,
    maxRepairAttempts: 3,
    maxReplanAttempts: 2,
  })

  assert.equal(block.repair.used, 0)
  assert.equal(block.repair.remaining, 3)
  assert.notEqual(block.repair.used, null)
})

test('the snapshot wiring reads the durable counters against the engine caps', () => {
  const block = forgeRepairBudgetFromEvidence({ repairAttempts: 1, replanAttempts: 2 })

  assert.deepEqual(block.repair, { used: 1, cap: 3, remaining: 2, exhausted: false })
  assert.deepEqual(block.replan, { used: 2, cap: 2, remaining: 0, exhausted: true })
})

test('the snapshot wiring keeps an absent counter unknown rather than zero', () => {
  const block = forgeRepairBudgetFromEvidence({})

  assert.equal(block.repair.used, null)
  assert.equal(block.repair.cap, 3)
  assert.equal(block.repair.remaining, null)
  assert.notEqual(block.repair.remaining, 0)

  assert.equal(block.replan.used, null)
  assert.equal(block.replan.cap, 2)
  assert.equal(block.replan.remaining, null)
})

test('ForgeVisibilitySnapshot carries the repairBudget field', () => {
  const block = forgeRepairBudgetFromEvidence({ repairAttempts: 0, replanAttempts: 0 })
  const field: ForgeVisibilitySnapshot['repairBudget'] = block

  assert.deepEqual(field, block)
})
