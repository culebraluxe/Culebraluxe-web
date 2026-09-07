import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  findingsFromArchitectEvidence,
  findingsMarker,
  leadShapePlan,
  shapeArchitectFindings,
  smithScopeForUnit,
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

