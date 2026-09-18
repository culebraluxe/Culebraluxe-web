import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  authoritativeLeadDecision,
  shapeArchitectFindings,
  smithScopeForUnit,
  validateLeadShapeChoice,
  type ArchitectFinding,
} from '../forge/forge-shaping'
import {
  handoffToFindings,
  parseArchitectHandoff,
  renderArchitectHandoff,
  type ArchitectHandoff,
} from '../forge/agents/architect-handoff'
import { smithWorkOrdersFromFindings } from '../forge/agents/architect/persist'

// ENG-FORGE-PROOF-SEAM-01 — A FINDING OWNS ITS PROOF.
//
// A finding that requires a NEW proof declares that proof path inside its own
// seams. Two units that add DISTINCT files under one shared test directory are
// not overlapping surfaces, so a story with two independent units can split;
// two units that would edit the SAME file stay refused.

function req(
  id: string,
  seams: string[],
  proofs?: string[],
  summary = id,
): ArchitectFinding {
  return { id, summary, required: true, seams, ...(proofs ? { proofs } : {}) }
}

test('two disjoint new proofs under one tests directory split and each unit carries its own proof', () => {
  const findings = [
    req('unit-a', ['workflow_app/tests'], ['workflow_app/tests/proof-a.test.ts']),
    req('unit-b', ['workflow_app/tests'], ['workflow_app/tests/proof-b.test.ts']),
  ]
  const decision = shapeArchitectFindings({ findings })
  assert.equal(decision.mode, 'SPLIT')
  assert.equal(decision.units.length, 2)

  const unitA = decision.units.find((u) => u.findingIds.includes('unit-a'))
  const unitB = decision.units.find((u) => u.findingIds.includes('unit-b'))
  assert.ok(unitA && unitB, 'each finding owns one bounded unit')
  assert.ok(unitA.seams.includes('workflow_app/tests/proof-a.test.ts'), 'unit A is handed its own proof')
  assert.ok(!unitA.seams.includes('workflow_app/tests/proof-b.test.ts'), 'unit A does not own the sibling proof')
  assert.ok(unitB.seams.includes('workflow_app/tests/proof-b.test.ts'), 'unit B is handed its own proof')

  const scopeA = smithScopeForUnit(decision, unitA.id)
  assert.ok(scopeA.includes('workflow_app/tests/proof-a.test.ts'))
})

test('a required proof outside its finding seams is refused at shaping by finding id', () => {
  const decision = shapeArchitectFindings({
    findings: [req('bad-proof', ['workflow_app/forge/forge-shaping.ts'], ['workflow_app/tests/orphan.test.ts'])],
  })
  assert.equal(decision.mode, 'HOLD')
  assert.ok(decision.reason.includes('bad-proof'), 'the refusal names the finding')
  assert.ok(decision.reason.includes('workflow_app/tests/orphan.test.ts'), 'the refusal names the proof')
})

test('two findings editing the same file still overlap and are refused', () => {
  const findings = [
    req('same-a', ['workflow_app/tests'], ['workflow_app/tests/same.test.ts']),
    req('same-b', ['workflow_app/tests'], ['workflow_app/tests/same.test.ts']),
  ]
  const decision = shapeArchitectFindings({ findings })
  assert.equal(decision.mode, 'SINGLE')
  assert.equal(decision.units.length, 1)
  assert.notEqual(authoritativeLeadDecision(findings).decision, 'SPLIT')
  assert.equal(validateLeadShapeChoice({ findings, choice: 'SPLIT', splitCount: 2 }).ok, false)
})

test('the architect handoff carries a finding proof into the work order', () => {
  const handoff: ArchitectHandoff = {
    version: 1,
    baseRef: 'deadbeef',
    findings: [
      {
        id: 'proof-owner',
        required: true,
        summary: 'owns its proof',
        preconditions: [],
        scope: ['workflow_app/tests'],
        proofs: ['workflow_app/tests/proof-seam.test.ts'],
        postconditions: [],
        classes: [],
        risks: [],
        hint: 'SAME_UNIT',
      },
    ],
  }
  const parsed = parseArchitectHandoff(renderArchitectHandoff(handoff))
  assert.ok(parsed, 'the handoff parses back')
  assert.deepEqual(parsed.findings[0].proofs, ['workflow_app/tests/proof-seam.test.ts'])
  const findings = handoffToFindings(parsed)
  assert.deepEqual(findings[0].proofs, ['workflow_app/tests/proof-seam.test.ts'])
  const order = smithWorkOrdersFromFindings(['proof-owner'], parsed)
  assert.ok(order.includes('workflow_app/tests/proof-seam.test.ts'), 'the work order names the proof')
})
