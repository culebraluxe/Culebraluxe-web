import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  DEFAULT_REPAIR_BUDGET,
  routeQaResult,
  type QaDisposition,
  type RepairAttemptState,
} from '../forge/qa-repair-policy'

const none = (): RepairAttemptState => ({ repairAttempts: 0, replanAttempts: 0 })

function route(verdict: 'PASS' | 'FAIL', disposition: QaDisposition | null | undefined, state: RepairAttemptState = none()) {
  return routeQaResult({ verdict, disposition, state })
}

test('QA PASS routes to the success path (never repair/replan/hold)', () => {
  const r = route('PASS', 'REPAIR', { repairAttempts: 2, replanAttempts: 1 })
  assert.deepEqual(r, { action: 'pass' })
})

test('QA FAIL + REPAIR within budget routes to Smith and increments the durable repair count', () => {
  const r = route('FAIL', 'REPAIR', { repairAttempts: 0, replanAttempts: 0 })
  assert.deepEqual(r, { action: 'smith', repairAttempts: 1 })
})

test('repeated in-budget repairs increment durably and never self-terminate', () => {
  let state = none()
  for (let i = 1; i <= DEFAULT_REPAIR_BUDGET.maxRepairAttempts; i++) {
    const r = route('FAIL', 'REPAIR', state)
    assert.equal(r.action, 'smith')
    if (r.action === 'smith') state = { ...state, repairAttempts: r.repairAttempts }
  }
  assert.equal(state.repairAttempts, 3)
})

test('QA FAIL + REPAIR exhaustion routes to HOLD (never an extra retry)', () => {
  const exhausted = { repairAttempts: DEFAULT_REPAIR_BUDGET.maxRepairAttempts, replanAttempts: 0 }
  const r = route('FAIL', 'REPAIR', exhausted)
  assert.equal(r.action, 'hold')
  assert.match((r as { action: 'hold'; reason: string }).reason, /Repair budget exhausted/)
})

test('QA FAIL + REPLAN within budget routes to Architect and increments the durable replan count', () => {
  const r = route('FAIL', 'REPLAN', { repairAttempts: 1, replanAttempts: 0 })
  assert.deepEqual(r, { action: 'architect', replanAttempts: 1 })
})

test('QA FAIL + REPLAN exhaustion routes to HOLD (never an extra replan)', () => {
  const exhausted = { repairAttempts: 0, replanAttempts: DEFAULT_REPAIR_BUDGET.maxReplanAttempts }
  const r = route('FAIL', 'REPLAN', exhausted)
  assert.equal(r.action, 'hold')
  assert.match((r as { action: 'hold'; reason: string }).reason, /Replan budget exhausted/)
})

test('QA FAIL + ESCALATE routes to HOLD (operator/Lead), never to a repair lane', () => {
  const r = route('FAIL', 'ESCALATE', none())
  assert.equal(r.action, 'hold')
})

test('QA FAIL with a MISSING disposition fails closed into HOLD (cannot accidentally succeed)', () => {
  const r = route('FAIL', undefined, none())
  assert.equal(r.action, 'hold')
})

test('QA FAIL with an INVALID disposition fails closed into HOLD', () => {
  const r = route('FAIL', 'NOT_A_DISPOSITION' as QaDisposition, none())
  assert.equal(r.action, 'hold')
})

test('decision depends only on the supplied durable state (restart/recovery neutral)', () => {
  const a = route('FAIL', 'REPAIR', { repairAttempts: 1, replanAttempts: 0 })
  const b = route('FAIL', 'REPAIR', { repairAttempts: 1, replanAttempts: 0 })
  assert.deepEqual(a, b)
  // A fresh process reconstructing the same durable counts yields the same route.
  const rebuilt = route('FAIL', 'REPAIR', { repairAttempts: 1, replanAttempts: 0 })
  assert.deepEqual(rebuilt, a)
})
