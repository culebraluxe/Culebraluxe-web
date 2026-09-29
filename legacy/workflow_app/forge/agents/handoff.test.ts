import assert from 'node:assert/strict'
import { test } from 'node:test'
import { parseArchitectHandoff, handoffToFindings, lastMachineLine } from '@/legacy/workflow_app/forge/agents/architect-handoff'
import { benchIntentErrors } from '@/legacy/workflow_app/forge/forge-lead-routing'
import { ROLE_EVIDENCE_MAP } from '@/legacy/workflow_app/forge/agents/evidence-map'

test('handoff maps scope onto live seams', () => {
  const raw = `prose\nFORGE_ARCHITECT_HANDOFF: {"version":1,"baseRef":"abc","findings":[{"id":"F1","required":true,"summary":"x","preconditions":["p"],"scope":["legacy/workflow_app/forge/a.ts"],"postconditions":["q"],"classes":["T"],"risks":["r"],"hint":"SAME_UNIT"}]}`
  const handoff = parseArchitectHandoff(raw)
  assert.ok(handoff)
  const findings = handoffToFindings(handoff!)
  assert.equal(findings[0].seams[0], 'legacy/workflow_app/forge/a.ts')
  assert.equal(findings[0].required, true)
})

test('last machine line survives after prose', () => {
  const raw = `FORGE_FINDINGS_JSON: []\nFORGE_FINDINGS_JSON: [{"id":"F1"}]`
  assert.equal(lastMachineLine(raw, 'FORGE_FINDINGS_JSON:'), 'FORGE_FINDINGS_JSON: [{"id":"F1"}]')
})

test('bench HOLD blocks SMITH', () => {
  assert.ok(benchIntentErrors('SMITH', 'HOLD').length)
  assert.equal(benchIntentErrors('HOLD', 'HOLD').length, 0)
  assert.ok(benchIntentErrors('SPLIT', 'SOLO').length)
})

test('evidence map covers six roles plus classifier', () => {
  const roles = ROLE_EVIDENCE_MAP.map((r) => r.role)
  for (const need of ['scout', 'architect', 'lead_pre', 'smith', 'assay', 'dev_ops', 'failure_classifier']) {
    assert.ok(roles.includes(need as (typeof roles)[number]), need)
  }
})
