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
  splitChildAssignment,
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

test('split-child handoff uses the ENGINE 0-based branch index', () => {
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
  // engine.ts forks `for (let i = 0; i < count; i++)` and sends `splitBranchIndex: i`
  // — index 0 IS the first child (a 1-based reading silently orphaned it).
  assert.equal(splitBranchIndexFromForm({ splitBranchIndex: 0 }), 0)
  assert.equal(splitBranchIndexFromForm({ splitBranchIndex: 1 }), 1)
  assert.equal(assignmentForSplitBranch(proposal, 0)?.id, 'a')
  assert.equal(assignmentForSplitBranch(proposal, 1)?.id, 'b')
  assert.equal(assignmentForSplitBranch(proposal, 9), null)
  // "absent" is null, never branch 0.
  assert.equal(splitBranchIndexFromForm({}), null)
  assert.equal(splitBranchIndexFromForm(null), null)
  assert.equal(splitBranchIndexFromForm({ splitBranchIndex: '' }), null)
  assert.equal(splitBranchIndexFromForm({ splitBranchIndex: -1 }), null)
})

test('a split child that cannot be tied to an assignment HOLDS (never invents scope)', () => {
  const proposal: LeadProposal = {
    version: 1, decision: 'SPLIT', size: 'LARGE', sizeReason: 's', reason: 'r',
    assignments: [assignment('a', 'f1', ['workflow_app/forge/a.ts'])],
    mergeChecks: [proof],
  }
  // No index on the form -> refuse.
  const noIndex = splitChildAssignment({ proposal, formData: {} })
  assert.equal(noIndex.assignment, null)
  assert.match(noIndex.errors.join('\n'), /no splitBranchIndex/)
  // Index beyond the accepted assignments -> refuse.
  const outOfRange = splitChildAssignment({ proposal, formData: { splitBranchIndex: 1 } })
  assert.equal(outOfRange.assignment, null)
  assert.match(outOfRange.errors.join('\n'), /no accepted Lead assignment for split branch 1/)
  // No accepted proposal at all -> refuse.
  const noProposal = splitChildAssignment({ proposal: null, formData: { splitBranchIndex: 0 } })
  assert.equal(noProposal.assignment, null)
  assert.ok(noProposal.errors.length > 0)
  // Resolvable + engine slice agrees -> allow.
  const ok = splitChildAssignment({
    proposal,
    formData: { splitBranchIndex: 0, splitBranch: { id: 'a' } },
  })
  assert.equal(ok.assignment?.id, 'a')
  assert.deepEqual(ok.errors, [])
})

test('an engine slice that disagrees with the accepted proposal is refused, not guessed', () => {
  const proposal: LeadProposal = {
    version: 1, decision: 'SPLIT', size: 'LARGE', sizeReason: 's', reason: 'r',
    assignments: [
      assignment('a', 'f1', ['workflow_app/forge/a.ts']),
      assignment('b', 'f2', ['workflow_app/forge/b.ts']),
    ],
    mergeChecks: [proof],
  }
  // Off-by-one class: the engine says 'b' where the accepted proposal puts 'a'.
  const mismatch = splitChildAssignment({
    proposal,
    formData: { splitBranchIndex: 0, splitBranch: { id: 'b' } },
  })
  assert.equal(mismatch.assignment?.id, 'a', 'the accepted proposal wins for RESOLUTION...')
  assert.match(mismatch.errors.join('\n'), /refusing rather than guessing/, '...but the disagreement is a HOLD')
})
