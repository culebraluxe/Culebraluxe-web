import { test } from 'node:test'
import assert from 'node:assert/strict'
import { projectForgeGateFacts } from '../forge/forge-facts'
import type { ForgeGateEvidence } from '../forge/forge-facts'

// ENG-FORGE-V11-S1 — engine-facing QA-failure routing facts. projectForgeGateFacts
// computes qaRepairEligible / qaReplanEligible from the QA disposition + the
// durable observer counts so the engine graph routes REPAIR/REPLAN while in
// budget and everything else (ESCALATE / missing / invalid / exhausted) to HOLD.

function facts(evidence: ForgeGateEvidence): Record<string, unknown> {
  return projectForgeGateFacts(evidence) as unknown as Record<string, unknown>
}

test('QA PASS is never repair/replan eligible', () => {
  const f = facts({ qaPassed: true })
  assert.equal(f.qaRepairEligible, false)
  assert.equal(f.qaReplanEligible, false)
})

test('FAIL + REPAIR within budget -> repair eligible (Smith), not replan', () => {
  const f = facts({ qaPassed: false, disposition: 'REPAIR', repairAttempts: 1, replanAttempts: 0 })
  assert.equal(f.qaRepairEligible, true)
  assert.equal(f.qaReplanEligible, false)
})

test('FAIL + REPAIR budget exhausted -> neither eligible (engine routes to HOLD)', () => {
  const f = facts({ qaPassed: false, disposition: 'REPAIR', repairAttempts: 3, replanAttempts: 0 })
  assert.equal(f.qaRepairEligible, false)
  assert.equal(f.qaReplanEligible, false)
})

test('FAIL + REPLAN within budget -> replan eligible (Architect), not repair', () => {
  const f = facts({ qaPassed: false, disposition: 'REPLAN', repairAttempts: 0, replanAttempts: 1 })
  assert.equal(f.qaReplanEligible, true)
  assert.equal(f.qaRepairEligible, false)
})

test('FAIL + REPLAN budget exhausted -> neither eligible (engine routes to HOLD)', () => {
  const f = facts({ qaPassed: false, disposition: 'REPLAN', repairAttempts: 0, replanAttempts: 2 })
  assert.equal(f.qaReplanEligible, false)
  assert.equal(f.qaRepairEligible, false)
})

test('FAIL + ESCALATE -> neither eligible (engine routes to HOLD)', () => {
  const f = facts({ qaPassed: false, disposition: 'ESCALATE', repairAttempts: 0, replanAttempts: 0 })
  assert.equal(f.qaReplanEligible, false)
  assert.equal(f.qaRepairEligible, false)
})

test('FAIL + missing disposition -> neither eligible (fail closed to HOLD)', () => {
  const f = facts({ qaPassed: false, disposition: undefined, repairAttempts: 0, replanAttempts: 0 })
  assert.equal(f.qaRepairEligible, false)
  assert.equal(f.qaReplanEligible, false)
})
