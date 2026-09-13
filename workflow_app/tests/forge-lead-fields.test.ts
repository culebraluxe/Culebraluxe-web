import assert from 'node:assert/strict'
import { test } from 'node:test'

import { resolveLeadProposal } from '../forge/lead-proposal-resolve'
import { LeadAgent } from '../forge/agents/role-agents'
import type { ForgeRoleContract } from '../../db/forge-role-contract'
import type { ForgeRolePlan } from '../../db/forge-role-plan'
import type { RoutingContext } from '../forge/forge-lead-routing'
import type { ForgeGateEvidence } from '../forge/forge-facts'

// ---------------------------------------------------------------------------
// LEAD DECISION CHANNEL — fields first, reply second, ONE seat.
//
// Both external reviews found the same defect independently: the DB contract rows
// (migrations 170/171) were a FALLBACK. LeadAgent.collect parsed the reply, set
// leadDecision, and the runner then skipped its field-aware review entirely — so a
// dropped marker could still cost a decision the database already held, which is the
// failure that cost 18 minutes on 2026-09-13.
//
// These tests need no database: the rows are plain objects of the same shape the
// readers return. They are the tests Grok's review called out as missing ("there is
// no test that imports getForgeRoleContract, getForgeRolePlan or leadProposalFromFields").
// ---------------------------------------------------------------------------

const PROOF = 'node --import tsx --test workflow_app/tests/forge-lead-fields.test.ts'
const SEAM = 'workflow_app/forge/forge-lead-routing.ts'
const FINDING = 'F1'

const context = (over: Partial<RoutingContext> = {}): RoutingContext => ({
  findings: [{ id: FINDING, required: true, hint: 'SAME_UNIT', seams: [SEAM] }],
  evidenceRefs: ['architect_brief'],
  splitEnabled: false,
  maxSmiths: 1,
  allowedProofs: [PROOF],
  ...over,
})

const contract = (over: Partial<ForgeRoleContract> = {}): ForgeRoleContract => ({
  decision: 'SMITH',
  size: 'SMALL',
  sizeReason: 'one bounded change',
  reason: 'the finding is required',
  assignmentCount: 1,
  findingIds: [FINDING],
  mergeChecks: [PROOF],
  surfaceScope: [SEAM],
  attempt: 1,
  ...over,
})

const assignment = {
  id: 'A1',
  findingIds: [FINDING],
  dependsOn: [],
  evidenceRefs: ['architect_brief'],
  reasoning: 'the decision row says SMITH',
  features: {
    semanticSurface: 1,
    dependencyDepth: 1,
    uncertainty: 1,
    contextBurden: 1,
    proofBurden: 1,
    coupling: 1,
    changeNovelty: 1,
    workerFit: 1,
  },
  plan: {
    size: 'SMALL',
    chunks: [
      {
        id: 1,
        outcome: 'the recorded decision routes',
        surface: [SEAM],
        invariant: 'reply text cannot override a recorded row',
        proof: PROOF,
        dependsOn: [],
      },
    ],
  },
}

const plan = (over: Partial<ForgeRolePlan> = {}): ForgeRolePlan => ({
  size: 'SMALL',
  assignments: [assignment],
  ...over,
})

/** The reply line a model would emit — the FALLBACK channel. */
const replyWithProposal = (): string =>
  `prose about the story\nLEAD_ROUTING: ${JSON.stringify({
    version: 1,
    decision: 'SMITH',
    size: 'SMALL',
    sizeReason: 'one bounded change',
    reason: 'the finding is required',
    mergeChecks: [PROOF],
    assignments: [assignment],
  })}`

const why = (review: { ok: boolean; errors?: string[] }): string =>
  review.ok ? '' : (review.errors ?? []).join('; ')

test('fields-only: the recorded rows route with NO LEAD_ROUTING line anywhere', () => {
  const review = resolveLeadProposal({
    raw: 'prose only — the model emitted no machine line at all',
    contract: contract(),
    plan: plan(),
    context: context(),
  })

  assert.equal(review.ok, true, why(review))
  if (review.ok) assert.equal(review.proposal.decision, 'SMITH')
})

test('fields WIN: a recorded HOLD beats a leftover SMITH line in the reply', () => {
  const review = resolveLeadProposal({
    raw: replyWithProposal(),
    contract: contract({ decision: 'HOLD', assignmentCount: null, mergeChecks: [] }),
    plan: plan(),
    context: context(),
  })

  assert.equal(review.ok, true, why(review))
  if (review.ok) {
    assert.equal(review.proposal.decision, 'HOLD', 'the row is the authority, not the reply')
    assert.deepEqual(review.proposal.assignments, [])
  }
})

test('fallback intact: with no recorded row the reply still routes (older runs keep working)', () => {
  const review = resolveLeadProposal({
    raw: replyWithProposal(),
    contract: null,
    plan: null,
    context: context(),
  })

  assert.equal(review.ok, true, why(review))
  if (review.ok) assert.equal(review.proposal.decision, 'SMITH')
})

test('a bench-cap refusal lands on deliverableRejection instead of returning silently', () => {
  const agent = new LeadAgent('lead_pre')
  const evidence: ForgeGateEvidence = {
    findings: [{ id: FINDING, summary: 'x', required: true, hint: 'SAME_UNIT', seams: [SEAM] }],
  }

  const out = agent.collect(evidence, replyWithProposal(), {
    ...context(),
    benchIntent: 'SOLO',
  })

  assert.equal(out.leadDecision, undefined, 'a capped route must not be recorded as decided')
  assert.match(String(out.deliverableRejection), /Bench intent is SOLO/)
})

test('an uncapped run still routes, and the decision reaches the evidence', () => {
  const agent = new LeadAgent('lead_pre')
  const evidence: ForgeGateEvidence = {
    findings: [{ id: FINDING, summary: 'x', required: true, hint: 'SAME_UNIT', seams: [SEAM] }],
  }

  const out = agent.collect(evidence, replyWithProposal(), context())

  assert.equal(out.deliverableRejection, undefined)
  assert.equal(out.leadDecision, 'SMITH')
})
