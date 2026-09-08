import { test } from 'node:test'
import assert from 'node:assert/strict'
import { forgeRoleNodePlan } from '../forge/forge-role-mapping'
import { findingsFromArchitectEvidence } from '../forge/forge-shaping'

// Scout evidence marshaling: research_scout must be told to emit
// FORGE_FINDINGS_JSON, and that marker must parse back into findings so they land
// in forge_workflow_evidence.findings (not just the notes blob). Architect then
// inherits real intel instead of GIGO.
test('scout plan asks for FORGE_FINDINGS_JSON so findings have a structured home', () => {
  const plan = forgeRoleNodePlan('research_scout')
  assert.equal(plan.lane, 'scout')
  assert.ok(
    plan.evidenceInstruction?.includes('FORGE_FINDINGS_JSON:'),
    'scout evidenceInstruction must request structured findings',
  )
})

test('scout-style FORGE_FINDINGS_JSON marker parses into findings', () => {
  const notes =
    '## research notes\n' +
    'The surface is the Forge park + RESEARCH flow.\n\n' +
    'FORGE_FINDINGS_JSON: [{"id":"scout-1","summary":"forge-executor owns the stopAfter park logic","required":false,"seams":["workflow_app/forge/forge-executor.ts"],"hint":"NOTE"}]\n'
  const findings = findingsFromArchitectEvidence(notes)
  assert.equal(findings.length, 1)
  assert.equal(findings[0]!.id, 'scout-1')
  assert.equal(findings[0]!.hint, 'NOTE')
  assert.deepEqual(findings[0]!.seams, ['workflow_app/forge/forge-executor.ts'])
})
