import assert from 'node:assert/strict'
import test from 'node:test'
import {
  FORGE_JUDGMENT_LAB_NODES,
  isForgeJudgmentLabNode,
} from '../forge/forge-judgment'
import {
  detectForgeDualWrite,
  parseForgeRoutingBrain,
  forgeRoutingBrainShouldFollowReducer,
} from '../forge/forge-routing-brain'
import {
  forgeNodeRequiresTypedGateEvidence,
  readLegacyMarkerGateEvidence,
  readTypedGateEvidence,
} from '../forge/forge-typed-evidence'
import { FORGE_HUMAN_GATE_NODES } from '../forge/forge-executor'
import { forgeVisibilityEquality } from '../forge/forge-visibility'

test('V11 judgment-lab nodes stay labeled but auto-run overnight', () => {
  for (const node of FORGE_JUDGMENT_LAB_NODES) {
    assert.equal(isForgeJudgmentLabNode(node), true)
    assert.equal(FORGE_HUMAN_GATE_NODES.has(node), false)
  }
  assert.equal(FORGE_HUMAN_GATE_NODES.has('hold'), true)
  assert.equal(FORGE_HUMAN_GATE_NODES.has('repair_requirements'), true)
  assert.equal(FORGE_HUMAN_GATE_NODES.has('smith'), false)
  assert.equal(FORGE_HUMAN_GATE_NODES.has('architect'), false)
  assert.equal(FORGE_HUMAN_GATE_NODES.has('qa_review'), false)
})

test('V11 typed evidence is preferred when present', () => {
  assert.equal(forgeNodeRequiresTypedGateEvidence('architect'), true)
  assert.equal(forgeNodeRequiresTypedGateEvidence('lead_pre'), true)
  const typed = readTypedGateEvidence({
    notes: 'FORGE_EVIDENCE_JSON: {"leadDecision":"SPLIT"}',
    testsSummary: null,
    gateEvidence: { leadDecision: 'SOLO' },
  })
  assert.deepEqual(typed, { leadDecision: 'SOLO' })
})

test('V11 legacy marker path remains available so overnight can advance', () => {
  const marked = readLegacyMarkerGateEvidence({
    notes: 'FORGE_EVIDENCE_JSON: {"scoutRequired":true}',
    testsSummary: null,
  })
  assert.equal(marked.scoutRequired, true)
})

test('V11 routing brain defaults to reducer and detects dual-write', () => {
  assert.equal(parseForgeRoutingBrain(undefined), 'reducer')
  assert.equal(parseForgeRoutingBrain('engine'), 'engine')
  assert.equal(forgeRoutingBrainShouldFollowReducer('reducer'), true)
  assert.equal(
    detectForgeDualWrite({
      storyId: 's1',
      reducerTouched: true,
      engineInstanceActive: true,
    }).ok,
    false,
  )
})

test('V11 SHA-chain equality flags', () => {
  const flags = forgeVisibilityEquality({
    candidateSha: 'aaaaaaaa',
    qaVerifiedSha: 'aaaaaaaa',
    publishedSha: 'aaaaaaaa',
    deployedSha: null,
    productionVerifiedSha: 'aaaaaaaa',
  })
  assert.equal(flags.candidateEqualsQa, true)
  assert.equal(flags.publishedEqualsCandidate, true)
  assert.equal(flags.productionEqualsPublished, true)
  assert.equal(flags.deployedEqualsPublished, false)
})
