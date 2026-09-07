import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  authoritativeLeadDecision,
  findingsFromArchitectEvidence,
  findingsMarker,
  leadShapePlan,
  shapeArchitectFindings,
  smithScopeForUnit,
  smithUnitForNode,
  validateLeadShapeChoice,
  type ArchitectFinding,
} from '../forge/forge-shaping'

// ENG-FORGE-SHAPE-01 — judgment layer between Architect discovery and Smith
// execution. Cases A-D mirror the SHAPE-01 work order acceptance scenarios.

function req(id: string, seams: string[], summary = id, hint?: ArchitectFinding['hint']): ArchitectFinding {
  return { id, summary, required: true, seams, hint }
}

test('SHAPE Case A: several observations, ONE seam -> SINGLE bounded Smith unit', () => {
  const decision = shapeArchitectFindings({
    findings: [
      req('f1', ['workflow_app/forge/'], 'false-success handling'),
      req('f2', ['workflow_app/forge/forge-facts.ts'], 'typed gate facts'),
      req('f3', ['workflow_app/forge/forge-role-mapping.ts'], 'directory scope normalize'),
    ],
  })
  assert.equal(decision.mode, 'SINGLE')
  assert.equal(decision.units.length, 1)
  assert.deepEqual([...decision.units[0].findingIds].sort(), ['f1', 'f2', 'f3'])
  assert.equal(decision.followUps.length, 0)
  const plan = leadShapePlan(decision)
  assert.equal(plan.decision, 'SMITH')
  assert.equal(plan.splitCount, null)
})

test('SHAPE Case B: two independent required seams -> SPLIT(2) bounded children', () => {
  const decision = shapeArchitectFindings({
    findings: [
      req('forge-layer', ['workflow_app/forge/forge-facts.ts'], 'forge seam fix'),
      req('harness-layer', ['agent-runtime/opencode/opencode-harness-adapter.ts'], 'harness seam fix'),
    ],
  })
  assert.equal(decision.mode, 'SPLIT')
  assert.equal(decision.units.length, 2)
  const plan = leadShapePlan(decision)
  assert.equal(plan.decision, 'SPLIT')
  assert.equal(plan.splitCount, 2)
})

test('SHAPE Case C: adjacent discovery never enlarges the active Smith unit', () => {
  const decision = shapeArchitectFindings({
    findings: [
      req('required-fix', ['workflow_app/forge/forge-facts.ts'], 'the required fix'),
      { id: 'adjacent-1', summary: 'adjacent defect in WBS board', required: false, seams: ['components/portal/wbs/'] },
      { id: 'adjacent-2', summary: 'adjacent defect in TECH cockpit', required: false, seams: ['app/tech/'] },
    ],
  })
  assert.equal(decision.mode, 'SINGLE')
  assert.equal(decision.units.length, 1)
  // Only the required finding is in Smith scope.
  assert.deepEqual(decision.units[0].findingIds, ['required-fix'])
  // Both adjacent findings are follow-up stories, NOT Smith scope.
  assert.equal(decision.followUps.length, 2)
  assert.deepEqual([...decision.followUps.map((f) => f.findingIds[0])].sort(), ['adjacent-1', 'adjacent-2'])
  const scope = smithScopeForUnit(decision, decision.units[0].id)
  assert.ok(scope.every((s) => !s.includes('components/portal/wbs')))
  assert.ok(scope.every((s) => !s.includes('app/tech')))
})
test('SHAPE Case D: a chosen unit owns only its own seams (repair stays narrow)', () => {
  const decision = shapeArchitectFindings({
    findings: [
      req('forge', ['workflow_app/forge/forge-facts.ts'], 'forge seam'),
      req('harness', ['agent-runtime/opencode/opencode-harness-adapter.ts'], 'harness seam'),
    ],
  })
  assert.equal(decision.mode, 'SPLIT')
  const forgeUnit = decision.units.find((u) => u.findingIds.includes('forge'))!.id
  const forgeScope = smithScopeForUnit(decision, forgeUnit)
  assert.ok(forgeScope.includes('workflow_app/forge/forge-facts.ts'))
  // Repair scope for one failed invariant must not reach the sibling seam.
  assert.ok(!forgeScope.some((s) => s.startsWith('agent-runtime/opencode')))
})

test('SHAPE: explicit SPLIT_CHILD is honored even when seams overlap (topology, not packet size)', () => {
  const decision = shapeArchitectFindings({
    findings: [
      req('a', ['workflow_app/forge/'], 'core change', 'SAME_UNIT'),
      req('b', ['workflow_app/forge/'], 'independent child', 'SPLIT_CHILD'),
    ],
  })
  assert.equal(decision.mode, 'SPLIT')
  assert.equal(decision.units.length, 2)
})

test('SHAPE: informational NOTE findings are notes, not follow-ups', () => {
  const decision = shapeArchitectFindings({
    findings: [
      req('fix', ['workflow_app/forge/forge-facts.ts'], 'required fix'),
      { id: 'info', summary: 'informational observation', required: false, seams: [], hint: 'NOTE' },
    ],
  })
  assert.equal(decision.notes.length, 1)
  assert.equal(decision.followUps.length, 0)
})

test('SHAPE: required HOLD stops execution; lead maps it to HOLD', () => {
  const decision = shapeArchitectFindings({
    findings: [req('ambiguous', ['workflow_app/forge/'], 'too ambiguous to execute', 'HOLD')],
  })
  assert.equal(decision.mode, 'HOLD')
  assert.deepEqual(decision.holds, ['ambiguous'])
  assert.equal(leadShapePlan(decision).decision, 'HOLD')
})

test('SHAPE integration: multi-seam Architect evidence (marker) is shaped before Smith', () => {
  // The architect emitted ONE packet with two required seams + two adjacent finds.
  const evidence = findingsMarker([
    req('forge-seam', ['workflow_app/forge/'], 'forge findings'),
    req('harness-seam', ['agent-runtime/opencode/opencode-harness-adapter.ts'], 'harness findings'),
    { id: 'adj', summary: 'adjacent TECH finding', required: false, seams: ['app/tech/'] },
    { id: 'info', summary: 'context note', required: false, seams: [], hint: 'NOTE' },
  ])
  const parsed = findingsFromArchitectEvidence(`Architect reviewed the story.\n${evidence}`)
  assert.equal(parsed.length, 4)
  const decision = shapeArchitectFindings({ findings: parsed })
  // Two independent required seams -> SPLIT; adjacent find -> follow-up; info -> note.
  assert.equal(decision.mode, 'SPLIT')
  assert.equal(decision.units.length, 2)
  assert.equal(decision.followUps.length, 1)
  assert.equal(decision.notes.length, 1)
  // Every unit finding is required; adjacency is quarantined out of units.
  assert.ok(decision.units.every((u) => u.findingIds.every((id) => id !== 'adj' && id !== 'info')))
})

// -- Authoritative shaping gate (dogfood anti-pattern, enforced in code) ------

test('SHAPE gate (dogfood regression): single SMITH over two independent seams is REFUSED', () => {
  const findings = [
    req('forge-layer', ['workflow_app/forge/forge-facts.ts'], 'forge seam fix'),
    req('harness-layer', ['agent-runtime/opencode/opencode-harness-adapter.ts'], 'harness seam fix'),
  ]
  const authoritative = authoritativeLeadDecision(findings)
  assert.equal(authoritative.decision, 'SPLIT')
  assert.equal(authoritative.splitCount, 2)
  const verdict = validateLeadShapeChoice({ findings, choice: 'SMITH' })
  assert.equal(verdict.ok, false)
  assert.ok(verdict.errors.some((e) => e.includes('independent required seams')))
})

test('SHAPE gate: correct SPLIT:2 is accepted; a wrong split count is refused', () => {
  const findings = [
    req('a', ['workflow_app/forge/'], 'forge seam'),
    req('b', ['agent-runtime/opencode/'], 'harness seam'),
  ]
  assert.equal(validateLeadShapeChoice({ findings, choice: 'SPLIT', splitCount: 2 }).ok, true)
  const wrong = validateLeadShapeChoice({ findings, choice: 'SPLIT', splitCount: 3 })
  assert.equal(wrong.ok, false)
})

test('SHAPE gate: adjacent discovery never forces a SPLIT (one required seam stays SINGLE)', () => {
  const findings = [
    req('fix', ['workflow_app/forge/forge-facts.ts'], 'required fix'),
    { id: 'adj', summary: 'adjacent', required: false, seams: ['app/tech/'] },
    { id: 'adj2', summary: 'adjacent 2', required: false, seams: ['components/portal/wbs/'] },
  ]
  const authoritative = authoritativeLeadDecision(findings)
  assert.equal(authoritative.decision, 'SMITH')
  assert.equal(authoritative.splitCount, null)
  assert.equal(validateLeadShapeChoice({ findings, choice: 'SMITH' }).ok, true)
})

test('SHAPE gate: a plain smith node cannot launch under a SPLIT/HOLD shape', () => {
  const multi = shapeArchitectFindings({
    findings: [req('a', ['workflow_app/forge/'], 'a'), req('b', ['agent-runtime/'], 'b')],
  })
  // SPLIT shape -> a plain `smith` (not a split child) must not run.
  const singleLaunch = smithUnitForNode(multi, 'smith')
  assert.equal(singleLaunch.unit, null)
  assert.ok(singleLaunch.error?.includes('SPLIT'))
  // Each split child binds to exactly one bounded unit by index.
  const child0 = smithUnitForNode(multi, 'smith_split_work', 0)
  const child1 = smithUnitForNode(multi, 'smith_split_work', 1)
  assert.ok(child0.unit?.findingIds.includes('a'))
  assert.ok(child1.unit?.findingIds.includes('b'))
})

test('SHAPE gate: split index out of range and HOLD cannot launch a smith unit', () => {
  const multi = shapeArchitectFindings({
    findings: [req('a', ['workflow_app/forge/'], 'a'), req('b', ['agent-runtime/'], 'b')],
  })
  assert.ok(smithUnitForNode(multi, 'smith_split_work', 9).error?.includes('outside'))
  const hold = shapeArchitectFindings({ findings: [req('h', ['x/'], 'too ambiguous', 'HOLD')] })
  assert.equal(smithUnitForNode(hold, 'smith').unit, null)
})

test('SHAPE gate: single cohesive unit launches one bounded smith/repair smith', () => {
  const single = shapeArchitectFindings({ findings: [req('core', ['workflow_app/forge/'], 'core')] })
  assert.equal(smithUnitForNode(single, 'smith').unit?.findingIds[0], 'core')
  assert.equal(smithUnitForNode(single, 'repair_smith').unit?.findingIds[0], 'core')
})

