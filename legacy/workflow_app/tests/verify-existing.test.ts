// ---------------------------------------------------------------------------
// ENG-FORGE-VERIFY-EXISTING-01 — the fence for the direct-Assay route.
//
// A story whose required findings already exist on the base is JUDGED, not
// re-authored: an accepted ASSAY route names the candidate sha and yields the
// assay arrangement. It is accepted ONLY when that sha is the candidate the
// runner observed on the pinned base; with nothing to verify the route is
// REFUSED, never passed. Ordinary authoring (SOLO/SMITH/SPLIT/HOLD) is untouched.
//
// The four test names below carry the assertion refs the acceptance clause maps
// to, so the Assay reader finds each one on a pass/fail marker line.
// ---------------------------------------------------------------------------
import assert from 'node:assert/strict'
import test from 'node:test'

import {
  reviewLeadProposal,
  type LeadProposal,
  type RoutingContext,
} from '@/legacy/workflow_app/forge/forge-lead-routing'
import { assayRouteArrangement } from '@/legacy/workflow_app/forge/agent-runtime-role-runner'

const PROOF = 'node --import tsx --test workflow_app/tests/verify-existing.test.ts'
const CANDIDATE = 'a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0'
const SEAM = 'legacy/workflow_app/forge/forge-lead-routing.ts'

function context(overrides: Partial<RoutingContext> = {}): RoutingContext {
  return {
    findings: [
      { id: 'lead-routing-assay-vocabulary', required: true, hint: 'SAME_UNIT', seams: [SEAM] },
    ],
    evidenceRefs: ['architect_brief'],
    splitEnabled: true,
    maxSmiths: 2,
    allowedProofs: [PROOF],
    gitObservedCandidate: { sha: CANDIDATE, onBaseRef: true },
    ...overrides,
  }
}

function assayProposal(overrides: Partial<LeadProposal> = {}): LeadProposal {
  return {
    version: 1,
    decision: 'ASSAY',
    size: 'SMALL',
    sizeReason: 'the candidate already exists on the base; only verification remains',
    reason: 'direct Assay verification of the existing candidate',
    verifyCandidate: CANDIDATE,
    assignments: [],
    mergeChecks: [PROOF],
    ...overrides,
  }
}

test('verify-existing:existing-work-routes-to-assay', () => {
  const review = reviewLeadProposal(assayProposal(), context())
  assert.equal(review.ok, true, review.ok ? '' : review.errors.join('; '))
  if (!review.ok) return
  assert.equal(review.proposal.decision, 'ASSAY')
  const arrangement = assayRouteArrangement(review, [PROOF])
  assert.ok(arrangement, 'an accepted ASSAY route yields an assay arrangement')
  assert.equal(arrangement.lane, 'assay')
  assert.equal(arrangement.node, 'qa_verify')
})

test('verify-existing:route-names-candidate-sha', () => {
  const review = reviewLeadProposal(assayProposal(), context())
  assert.equal(review.ok, true, review.ok ? '' : review.errors.join('; '))
  if (!review.ok) return
  assert.equal(review.proposal.verifyCandidate, CANDIDATE)
  const arrangement = assayRouteArrangement(review, [PROOF])
  assert.equal(arrangement?.candidateSha, CANDIDATE)
})

test('verify-existing:nothing-to-verify-refused', () => {
  // A named sha with no candidate observed on the base: nothing to verify.
  const noCandidate = reviewLeadProposal(assayProposal(), context({ gitObservedCandidate: null }))
  assert.equal(noCandidate.ok, false)

  // A candidate on the base that is not the sha the route names: nothing to verify.
  const mismatched = reviewLeadProposal(
    assayProposal({ verifyCandidate: '0'.repeat(40) }),
    context(),
  )
  assert.equal(mismatched.ok, false)

  // Zero required findings: nothing to verify, even with a candidate on the base.
  const noFindings = reviewLeadProposal(assayProposal(), context({ findings: [] }))
  assert.equal(noFindings.ok, false)

  // A malformed sha is never a route.
  const malformed = reviewLeadProposal(assayProposal({ verifyCandidate: 'not-a-sha' }), context())
  assert.equal(malformed.ok, false)
})

test('verify-existing:ordinary-authoring-unchanged', () => {
  const smith: LeadProposal = {
    version: 1,
    decision: 'SMITH',
    size: 'SMALL',
    sizeReason: 'one seam',
    reason: 'ordinary authoring',
    assignments: [
      {
        id: 'a',
        findingIds: ['lead-routing-assay-vocabulary'],
        dependsOn: [],
        evidenceRefs: ['architect_brief'],
        reasoning: 'smallest change',
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
              outcome: 'route added',
              surface: [SEAM],
              invariant: 'unchanged elsewhere',
              proof: PROOF,
              dependsOn: [],
            },
          ],
        },
      },
    ],
    mergeChecks: [PROOF],
  }
  const review = reviewLeadProposal(smith, context())
  assert.equal(review.ok, true, review.ok ? '' : review.errors.join('; '))
  if (!review.ok) return
  assert.equal(review.proposal.decision, 'SMITH')
  assert.equal(
    assayRouteArrangement(review, [PROOF]),
    null,
    'ordinary authoring produces no verify-existing arrangement',
  )

  // A HOLD route still validates as before and yields no arrangement.
  const hold = reviewLeadProposal(
    {
      version: 1,
      decision: 'HOLD',
      size: 'SMALL',
      sizeReason: 'blocked',
      reason: 'blocked',
      assignments: [],
      mergeChecks: [],
    },
    context(),
  )
  assert.equal(hold.ok, true)
  if (hold.ok) assert.equal(assayRouteArrangement(hold, [PROOF]), null)
})
