import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  LARGE_REQUIRES_RECUT,
  SPLIT_UNAVAILABLE,
  reviewLeadProposal,
  type LeadProposal,
  type RoutingContext,
} from '../forge/forge-lead-routing'
import { buildLeadRoutingDirective } from '../forge/forge-lead-routing-prompt'
import {
  assignmentForSplitBranch,
  splitBranchIndexFromForm,
} from '../forge/forge-split-handoff'

const proof = 'pnpm exec tsx --test workflow_app/tests/forge-lead-routing-split.test.ts'

function features(overrides: Partial<LeadProposal['assignments'][0]['features']> = {}) {
  return {
    semanticSurface: 1,
    dependencyDepth: 1,
    uncertainty: 1,
    contextBurden: 1,
    proofBurden: 1,
    coupling: 1,
    changeNovelty: 1,
    workerFit: 1,
    ...overrides,
  }
}

function assignment(
  id: string,
  findingId: string,
  surface: string[],
): LeadProposal['assignments'][0] {
  return {
    id,
    findingIds: [findingId],
    dependsOn: [],
    evidenceRefs: ['architect_brief'],
    reasoning: `do ${id}`,
    features: features(),
    plan: {
      size: 'SMALL',
      chunks: [
        {
          id: 1,
          outcome: `done ${id}`,
          surface,
          invariant: 'existing callers still compile',
          proof,
        },
      ],
    },
  }
}

function context(splitEnabled: boolean, maxSmiths: number): RoutingContext {
  return {
    findings: [
      { id: 'f1', required: true, seams: ['workflow_app/forge/a.ts'] },
      { id: 'f2', required: true, seams: ['workflow_app/forge/b.ts'] },
    ],
    evidenceRefs: ['architect_brief'],
    splitEnabled,
    maxSmiths,
    allowedProofs: [proof],
  }
}

function smithMedium(): LeadProposal {
  return {
    version: 1,
    decision: 'SMITH',
    size: 'MEDIUM',
    sizeReason: 'one coherent unit',
    reason: 'one smith',
    assignments: [assignment('a', 'f1', ['workflow_app/forge/a.ts'])],
    mergeChecks: [proof],
  }
}

test('SPLIT while disabled is rejected with an explicit recut reason', () => {
  const proposal: LeadProposal = {
    version: 1,
    decision: 'SPLIT',
    size: 'LARGE',
    sizeReason: 'two seams',
    reason: 'split',
    assignments: [
      assignment('a', 'f1', ['workflow_app/forge/a.ts']),
      assignment('b', 'f2', ['workflow_app/forge/b.ts']),
    ],
    mergeChecks: [proof],
  }
  const review = reviewLeadProposal(proposal, context(false, 1))
  assert.equal(review.ok, false)
  if (!review.ok) assert.ok(review.errors.includes(SPLIT_UNAVAILABLE))
})

test('LARGE + SMITH while split is off asks for a recut, not a fake split', () => {
  const proposal: LeadProposal = {
    ...smithMedium(),
    size: 'LARGE',
    sizeReason: 'honestly large',
    assignments: [assignment('a', 'f1', ['workflow_app/forge/a.ts'])],
  }
  // only one required finding covered — also unassigned f2, but the recut reason must appear
  const review = reviewLeadProposal(proposal, context(false, 1))
  assert.equal(review.ok, false)
  if (!review.ok) assert.ok(review.errors.includes(LARGE_REQUIRES_RECUT))
})

test('MEDIUM + one SMITH while split is off is still legal when findings fit', () => {
  const ctx: RoutingContext = {
    findings: [{ id: 'f1', required: true, seams: ['workflow_app/forge/a.ts'] }],
    evidenceRefs: ['architect_brief'],
    splitEnabled: false,
    maxSmiths: 1,
    allowedProofs: [proof],
  }
  const review = reviewLeadProposal(smithMedium(), ctx)
  assert.equal(review.ok, true)
})

test('directive names the cap when split is off', () => {
  const text = buildLeadRoutingDirective(context(false, 1))
  assert.match(text, /SPLIT is NOT available this run/)
  assert.doesNotMatch(text, /LARGE: SPLIT to 2/)
})

test('directive teaches SPLIT only when the runtime enables it', () => {
  const text = buildLeadRoutingDirective(context(true, 3))
  assert.match(text, /SPLIT is available/)
  assert.match(text, /maxSmiths=3/)
})

test('split-child handoff maps 1-based branch index onto assignments', () => {
  const proposal: LeadProposal = {
    version: 1,
    decision: 'SPLIT',
    size: 'LARGE',
    sizeReason: 'two seams',
    reason: 'split',
    assignments: [
      assignment('a', 'f1', ['workflow_app/forge/a.ts']),
      assignment('b', 'f2', ['workflow_app/forge/b.ts']),
    ],
    mergeChecks: [proof],
  }
  assert.equal(assignmentForSplitBranch(proposal, 2)?.id, 'b')
  assert.equal(assignmentForSplitBranch(proposal, 9), null)
  assert.equal(splitBranchIndexFromForm({ splitBranchIndex: 2 }), 2)
  assert.equal(splitBranchIndexFromForm({ splitBranchIndex: 0 }), null)
})
