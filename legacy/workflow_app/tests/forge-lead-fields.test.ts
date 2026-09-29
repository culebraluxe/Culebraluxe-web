import assert from 'node:assert/strict'
import { test } from 'node:test'

import { resolveLeadProposal, leadProposalFromFields } from '@/legacy/workflow_app/forge/lead-proposal-resolve'
import { reviewLeadProposal, type RoutingContext } from '@/legacy/workflow_app/forge/forge-lead-routing'
import { LeadAgent } from '@/legacy/workflow_app/forge/agents/role-agents'
import type { ForgeRoleContract } from '@/legacy/db/forge-role-contract'
import type { ForgeRolePlan } from '@/legacy/db/forge-role-plan'
import type { ForgeGateEvidence } from '@/legacy/workflow_app/forge/forge-facts'

// ---------------------------------------------------------------------------
// LEAD PRE IS FIELDS-ONLY. This file is the regression fence for that law.
//
// Both the changelog and the judgment-lab rip say the same thing: "prefers rows" was
// a lie the model used, and chat JSON must not beat the database. On 2026-09-13 the
// lane was taught BOTH channels at once, the model obeyed the chat line, and
// forge_role_contract stayed empty on every run while the decision arrived as a marker.
//
// So these tests assert the ABSENCE of a reply channel, not just its lower priority:
// no rows means a HOLD with a reason, and a perfect LEAD_ROUTING line changes nothing.
// ---------------------------------------------------------------------------

const PROOF = 'node --import tsx --test workflow_app/tests/forge-lead-fields.test.ts'
const SEAM = 'legacy/workflow_app/forge/forge-lead-routing.ts'
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

/** A PERFECT routing line. Under the rip it must not reach the decision at all. */
const PERFECT_REPLY = `prose\nLEAD_ROUTING: ${JSON.stringify({
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

test('recorded rows route with no reply involved', () => {
  const review = resolveLeadProposal({ contract: contract(), plan: plan(), context: context() })
  assert.equal(review.ok, true, why(review))
  if (review.ok) assert.equal(review.proposal.decision, 'SMITH')
})

test('a recorded HOLD is a valid outcome and carries no assignments', () => {
  const review = resolveLeadProposal({
    contract: contract({ decision: 'HOLD', assignmentCount: null, mergeChecks: [] }),
    plan: plan(),
    context: context(),
  })
  assert.equal(review.ok, true, why(review))
  if (review.ok) {
    assert.equal(review.proposal.decision, 'HOLD')
    assert.deepEqual(review.proposal.assignments, [])
  }
})

test('NO ROWS is a refusal that names the fields channel, never a route', () => {
  const review = resolveLeadProposal({ contract: null, plan: null, context: context() })
  assert.equal(review.ok, false, 'an unwritten decision must never route')
  if (!review.ok) {
    assert.match(review.errors.join('; '), /recorded in fields/)
    assert.match(review.errors.join('; '), /forge-handoff\.mjs/)
    assert.doesNotMatch(review.errors.join('; '), /LEAD_ROUTING/, 'do not teach the dead marker')
  }
})

test('a decision that needs a plan but has no chunk rows is refused, not invented', () => {
  const proposal = leadProposalFromFields(contract(), null)
  const review = reviewLeadProposal(proposal, context())
  assert.equal(review.ok, false)
  assert.match(review.errors.join('; '), /Malformed|No decision recorded|assignment/i)
})

test('LeadAgent.collect is a NO-OP for PRE: a perfect reply line sets nothing', () => {
  const evidence: ForgeGateEvidence = {
    findings: [{ id: FINDING, summary: 'x', required: true, hint: 'SAME_UNIT', seams: [SEAM] }],
  }
  const out = new LeadAgent('lead_pre').collect(evidence, PERFECT_REPLY, context())

  assert.equal(out.leadDecision, undefined, 'chat JSON must not set the routing decision')
  assert.equal(out.deliverableRejection, undefined, 'and it is not a refusal either — it is ignored')
})

test('the launch cap is enforced by the reviewer that owns the rule, with a reason', () => {
  const review = resolveLeadProposal({
    contract: contract(),
    plan: plan(),
    context: context({ benchIntent: 'SOLO' }),
  })
  assert.equal(review.ok, false)
  assert.match(review.errors.join('; '), /Bench intent is SOLO/)
})

test('with no cap, the same rows route', () => {
  const review = resolveLeadProposal({ contract: contract(), plan: plan(), context: context() })
  assert.equal(review.ok, true, why(review))
})
