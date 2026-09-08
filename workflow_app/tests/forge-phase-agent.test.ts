import { test } from 'node:test'
import assert from 'node:assert/strict'
import { ForgePhaseAgent } from '../forge/agents/forge-phase-agent'
import {
  ArchitectAgent,
  ScoutAgent,
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

test('forgeAgentFor resolves concrete role subclasses by lane', () => {
  assert.ok(forgeAgentFor('research_scout') instanceof ScoutAgent)
  assert.ok(forgeAgentFor('research_architect') instanceof ArchitectAgent)
})
