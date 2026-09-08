import { test } from 'node:test'
import assert from 'node:assert/strict'
import { ForgePhaseAgent } from '../forge/agents/forge-phase-agent'
import {
  ArchitectAgent,
  DevOpsAgent,
  LeadAgent,
  QAAgent,
  ScoutAgent,
  SmithAgent,
  forgeAgentFor,
} from '../forge/agents/role-agents'

const FINDINGS_NOTES =
  'research notes\n' +
  'FORGE_FINDINGS_JSON: [{"id":"s1","summary":"forge-executor owns the park logic","required":false,"seams":["workflow_app/forge/forge-executor.ts"],"hint":"NOTE"}]\n'

test('research_scout is a scout agent declaring the scout-packet deliverable', () => {
  const agent = new ForgePhaseAgent('research_scout')
  assert.equal(agent.isScout, true)
  assert.equal(agent.deliverableKind(), 'scout-packet')
})

test('research_architect is an architect agent declaring the architect-plan deliverable', () => {
  const agent = new ForgePhaseAgent('research_architect')
  assert.equal(agent.isArchitect, true)
  assert.equal(agent.deliverableKind(), 'architect-plan')
})

test('scout agent marshals FORGE_FINDINGS_JSON into evidence.findings and builds a bounded packet', () => {
  const agent = new ForgePhaseAgent('research_scout')
  const evidence: { findings?: unknown[] } = {}
  agent.marshalFindings(evidence as never, FINDINGS_NOTES)
  assert.equal((evidence.findings as unknown[])?.length, 1)
  const packet = agent.scoutPacket(FINDINGS_NOTES)
  assert.ok(packet?.startsWith('Scout research packet (engine node research_scout):'))
})

test('missingDeliverables flags a scout with no findings/packet', () => {
  const agent = new ForgePhaseAgent('research_scout')
  const missing = agent.missingDeliverables({} as never, '', false)
  assert.ok(missing.includes('scout-packet'))
})

test('storyDeliverable centralizes the write-on-exit story field', () => {
  const scout = new ForgePhaseAgent('research_scout')
  const arch = new ForgePhaseAgent('research_architect')
  const smith = forgeAgentFor('smith') as SmithAgent
  assert.equal(scout.storyDeliverable('raw output')?.field, 'context_refs')
  assert.equal(arch.storyDeliverable('raw output')?.field, 'architect_brief')
  assert.equal(scout.storyDeliverable('') , null)
  assert.equal(smith.storyDeliverable('raw'), null)
})

test('forgeAgentFor resolves concrete role subclasses by lane', () => {
  assert.ok(forgeAgentFor('research_scout') instanceof ScoutAgent)
  assert.ok(forgeAgentFor('research_architect') instanceof ArchitectAgent)
})

const ev = (x: unknown) => x as never

test('lead flavor requires a decision', () => {
  const lead = forgeAgentFor('lead_pre') as LeadAgent
  assert.deepEqual(lead.missingDeliverables(ev({}) as never, '', false), ['lead-decision'])
  assert.deepEqual(lead.missingDeliverables(ev({ leadDecision: 'SMITH' }) as never, '', false), [])
})

test('smith flavor requires a candidate SHA', () => {
  const smith = forgeAgentFor('smith') as SmithAgent
  assert.deepEqual(smith.missingDeliverables(ev({}) as never, '', false), ['smith-candidate'])
  assert.deepEqual(smith.missingDeliverables(ev({ candidateSha: 'abc123' }) as never, '', false), [])
})

test('qa flavor treats PASS and FAIL both as a verdict', () => {
  const qa = forgeAgentFor('qa_verify') as QAAgent
  assert.deepEqual(qa.missingDeliverables(ev({}) as never, '', false), ['qa-verdict'])
  assert.deepEqual(qa.missingDeliverables(ev({ qaPassed: false }) as never, '', false), [])
  assert.deepEqual(qa.missingDeliverables(ev({ qaPassed: true }) as never, '', false), [])
})

test('dev_ops flavor requires a release/production receipt', () => {
  const ops = forgeAgentFor('production_smoke') as DevOpsAgent
  assert.deepEqual(ops.missingDeliverables(ev({}) as never, '', false), ['devops-receipt'])
  assert.deepEqual(
    ops.missingDeliverables(ev({ productionVerificationReceipt: 'r-1' }) as never, '', false),
    [],
  )
})

test('routingDecisionMissing: research_architect requires a valid research_disposition', () => {
  const arch = forgeAgentFor('research_architect') as ArchitectAgent
  assert.equal(arch.routingDecisionMissing(ev({}) as never), 'research_disposition')
  assert.equal(arch.routingDecisionMissing(ev({ researchDisposition: 'BOGUS' }) as never), 'research_disposition')
  assert.equal(arch.routingDecisionMissing(ev({ researchDisposition: 'IMPLEMENT' }) as never), null)
  assert.equal(arch.routingDecisionMissing(ev({ researchDisposition: 'ARCHIVE' }) as never), null)
})

test('routingDecisionMissing: lead requires a valid lead_decision (+splitCount when SPLIT)', () => {
  const lead = forgeAgentFor('lead_pre') as LeadAgent
  assert.equal(lead.routingDecisionMissing(ev({}) as never), 'lead_decision')
  assert.equal(lead.routingDecisionMissing(ev({ leadDecision: 'SHRED' }) as never), 'lead_decision')
  assert.equal(lead.routingDecisionMissing(ev({ leadDecision: 'SPLIT' }) as never), 'lead_decision.splitCount')
  assert.equal(lead.routingDecisionMissing(ev({ leadDecision: 'SPLIT', splitCount: 3 }) as never), null)
  assert.equal(lead.routingDecisionMissing(ev({ leadDecision: 'SMITH' }) as never), null)
  assert.equal(lead.routingDecisionMissing(ev({ leadDecision: 'SOLO' }) as never), null)
})

test('routingDecisionMissing: non-routing nodes have no requirement', () => {
  const scout = new ForgePhaseAgent('research_scout')
  const smith = forgeAgentFor('smith') as SmithAgent
  assert.equal(scout.routingDecisionMissing(ev({}) as never), null)
  assert.equal(smith.routingDecisionMissing(ev({}) as never), null)
})
