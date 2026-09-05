import assert from 'node:assert/strict'
import test from 'node:test'
import {
  assertNotJudgmentLabAutoRun,
  ForgeJudgmentAutoRunError,
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
  missingTypedGateHoldEvidence,
  readLegacyMarkerGateEvidence,
  readTypedGateEvidence,
} from '../forge/forge-typed-evidence'
import { FORGE_HUMAN_GATE_NODES } from '../forge/forge-executor'
import { forgeVisibilityEquality } from '../forge/forge-visibility'

test('V11 judgment-lab nodes are not auto-runnable', () => {
  for (const node of FORGE_JUDGMENT_LAB_NODES) {
    assert.equal(isForgeJudgmentLabNode(node), true)
    assert.throws(() => assertNotJudgmentLabAutoRun(node), ForgeJudgmentAutoRunError)
  }
  assert.doesNotThrow(() => assertNotJudgmentLabAutoRun('smith'))
  assert.doesNotThrow(() => assertNotJudgmentLabAutoRun('qa_verify'))
})

test('V11 executor parks judgment nodes with HOLD/requirements', () => {
  assert.equal(FORGE_HUMAN_GATE_NODES.has('architect'), true)
  assert.equal(FORGE_HUMAN_GATE_NODES.has('repair_architect'), true)
  assert.equal(FORGE_HUMAN_GATE_NODES.has('research_architect'), true)
  assert.equal(FORGE_HUMAN_GATE_NODES.has('qa_review'), true)
  assert.equal(FORGE_HUMAN_GATE_NODES.has('smith'), false)
})

test('V11 typed evidence is required for scout/architect/lead/devops routers', () => {
  assert.equal(forgeNodeRequiresTypedGateEvidence('architect'), true)
  assert.equal(forgeNodeRequiresTypedGateEvidence('lead_pre'), true)
  assert.equal(forgeNodeRequiresTypedGateEvidence('feature_scout'), true)
  assert.equal(forgeNodeRequiresTypedGateEvidence('repair_devops'), true)
  assert.equal(forgeNodeRequiresTypedGateEvidence('smith'), false)
  assert.equal(forgeNodeRequiresTypedGateEvidence('qa_verify'), false)
})

test('V11 typed evidence wins; missing typed evidence HOLDs', () => {
  const typed = readTypedGateEvidence({
    notes: 'FORGE_EVIDENCE_JSON: {"leadDecision":"SPLIT"}',
    testsSummary: null,
    gateEvidence: { leadDecision: 'SOLO' },
  })
  assert.deepEqual(typed, { leadDecision: 'SOLO' })
  assert.equal(
    readTypedGateEvidence({ notes: 'prose only', testsSummary: null }),
    null,
  )
  const hold = missingTypedGateHoldEvidence('lead_pre')
  assert.equal(hold.failureClass, 'UNKNOWN_CAUSE')
  assert.equal(hold.resumeTarget, 'LEAD')
})

test('V11 legacy marker path remains test-tagged fallback only', () => {
  const marked = readLegacyMarkerGateEvidence({
    notes: 'FORGE_EVIDENCE_JSON: {"scoutRequired":true}',
    testsSummary: null,
  })
  assert.equal(marked.scoutRequired, true)
})

test('V11 routing brain defaults to reducer and detects dual-write', () => {
  assert.equal(parseForgeRoutingBrain(undefined), 'reducer')
  assert.equal(parseForgeRoutingBrain(''), 'reducer')
  assert.equal(parseForgeRoutingBrain('engine'), 'engine')
  assert.equal(forgeRoutingBrainShouldFollowReducer('reducer'), true)
  assert.equal(forgeRoutingBrainShouldFollowReducer('engine'), false)
  assert.equal(
    detectForgeDualWrite({
      storyId: 's1',
      reducerTouched: true,
      engineInstanceActive: true,
    }).ok,
    false,
  )
  assert.equal(
    detectForgeDualWrite({
      storyId: 's1',
      reducerTouched: true,
      engineInstanceActive: false,
    }).ok,
    true,
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
