import assert from 'node:assert/strict'
import test from 'node:test'
import { reviewLeadProposal, type LeadProposal, type RoutingContext } from '../forge/forge-lead-routing'

const proof = 'pnpm exec tsx --test workflow_app/tests/example.test.ts'

function solo(): { p: LeadProposal; context: RoutingContext } {
  return {
    context: {
      findings: [{ id: 'behavior', required: true, seams: ['db/contact.ts'] }],
      evidenceRefs: ['architect:behavior'],
      splitEnabled: true,
      maxSmiths: 8,
      allowedProofs: [proof],
    },
    p: {
      version: 1,
      decision: 'SOLO',
      size: 'SMALL',
      sizeReason: 'one behavior',
      reason: 'solo',
      assignments: [{
        id: 'a',
        findingIds: ['behavior'],
        dependsOn: [],
        evidenceRefs: ['architect:behavior'],
        reasoning: 'mapped',
        features: {
          semanticSurface: 1, dependencyDepth: 1, uncertainty: 1, contextBurden: 1,
          proofBurden: 1, coupling: 1, changeNovelty: 1, workerFit: 1,
        },
        plan: {
          size: 'SMALL',
          chunks: [{
            id: 1,
            outcome: 'ok',
            surface: ['db/contact.ts'],
            invariant: 'ok',
            proof,
          }],
        },
      }],
      mergeChecks: [proof],
    },
  }
}

test('scope alias is accepted as surface', () => {
  const { p, context } = solo()
  const chunk = p.assignments[0].plan.chunks[0] as { scope?: string[]; surface?: string[] }
  chunk.scope = ['db/contact.ts']
  delete chunk.surface
  const r = reviewLeadProposal(p, context)
  assert.equal(r.ok, true)
  if (r.ok) assert.deepEqual(r.proposal.assignments[0].plan.chunks[0].surface, ['db/contact.ts'])
})

test('Bench HOLD refuses SOLO', () => {
  const { p, context } = solo()
  context.benchIntent = 'HOLD'
  const r = reviewLeadProposal(p, context)
  assert.equal(r.ok, false)
  if (!r.ok) assert.match(r.errors.join('\n'), /Bench intent is HOLD/)
})

test('Bench SOLO refuses SPLIT', () => {
  const { p, context } = solo()
  p.decision = 'SPLIT'
  p.size = 'LARGE'
  context.findings.push({ id: 'other', required: true, seams: ['db/other.ts'] })
  const b = structuredClone(p.assignments[0])
  b.id = 'b'
  b.findingIds = ['other']
  b.plan.chunks[0].surface = ['db/other.ts']
  p.assignments.push(b)
  context.benchIntent = 'SOLO'
  const r = reviewLeadProposal(p, context)
  assert.equal(r.ok, false)
  if (!r.ok) assert.match(r.errors.join('\n'), /Bench intent is SOLO/)
})
