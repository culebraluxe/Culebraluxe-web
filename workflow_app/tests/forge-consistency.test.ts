import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  type StoryConsistencySnapshot,
  auditStoryConsistency,
} from '../forge/forge-consistency'

const base = (over: Partial<StoryConsistencySnapshot>): StoryConsistencySnapshot => ({
  storyId: 'S-1',
  storyStatus: 'Complete',
  evidence: null,
  run: null,
  engineNodesCompleted: [],
  openTaskCount: 0,
  ...over,
})

test('consistency: a clean complete story (candidate + assay + publish) has no violations', () => {
  const v = auditStoryConsistency(
    base({
      evidence: {
        candidateSha: 'a'.repeat(40),
        qaPassed: true,
        qaVerifiedSha: 'a'.repeat(40),
        publishedSha: 'a'.repeat(40),
        deployedSha: null,
      },
      run: { resultStatus: 'Complete' },
      engineNodesCompleted: ['smith'],
    }),
  )
  assert.deepEqual(v, [])
})

test('I1: engine-processed Complete with no candidate/QA is flagged', () => {
  const v = auditStoryConsistency(
    base({
      evidence: { candidateSha: null, qaPassed: null, qaVerifiedSha: null, publishedSha: null, deployedSha: null },
      engineNodesCompleted: ['feature_scout'],
    }),
  )
  assert.ok(v.some((x) => x.kind === 'complete-with-only-scout-evidence'))
})

test('I1: a legacy Complete story the engine never touched (no evidence) is NOT flagged', () => {
  const v = auditStoryConsistency(base({ evidence: null }))
  assert.ok(!v.some((x) => x.kind === 'complete-with-only-scout-evidence'))
})

test('I1: Complete with a real candidate is NOT scout-only', () => {
  const v = auditStoryConsistency(
    base({ evidence: { candidateSha: 'b'.repeat(40), qaPassed: null, qaVerifiedSha: null, publishedSha: null, deployedSha: null } }),
  )
  assert.ok(!v.some((x) => x.kind === 'complete-with-only-scout-evidence'))
})

test('I2: a non-terminal story with no open task and no candidate is parked', () => {
  const v = auditStoryConsistency(base({ storyStatus: 'Planned', openTaskCount: 0 }))
  assert.ok(v.some((x) => x.kind === 'in-progress-no-actionable-work'))
})

test('I2: an open task means there IS actionable work', () => {
  const v = auditStoryConsistency(base({ storyStatus: 'Planned', openTaskCount: 1 }))
  assert.ok(!v.some((x) => x.kind === 'in-progress-no-actionable-work'))
})

test('I3: a candidate with no Assay result is flagged', () => {
  const v = auditStoryConsistency(
    base({ evidence: { candidateSha: 'c'.repeat(40), qaPassed: null, qaVerifiedSha: null, publishedSha: null, deployedSha: null } }),
  )
  assert.ok(v.some((x) => x.kind === 'smith-candidate-without-assay'))
})

test('I3: an assayed candidate is not flagged', () => {
  const v = auditStoryConsistency(
    base({ evidence: { candidateSha: 'c'.repeat(40), qaPassed: true, qaVerifiedSha: 'c'.repeat(40), publishedSha: null, deployedSha: null } }),
  )
  assert.ok(!v.some((x) => x.kind === 'smith-candidate-without-assay'))
})

test('I4: a verified candidate never published is flagged', () => {
  const v = auditStoryConsistency(
    base({ evidence: { candidateSha: 'd'.repeat(40), qaPassed: true, qaVerifiedSha: 'd'.repeat(40), publishedSha: null, deployedSha: null } }),
  )
  assert.ok(v.some((x) => x.kind === 'clean-assay-unpublished'))
})

test('I4: a published candidate is not flagged', () => {
  const v = auditStoryConsistency(
    base({ evidence: { candidateSha: 'd'.repeat(40), qaPassed: true, qaVerifiedSha: 'd'.repeat(40), publishedSha: 'd'.repeat(40), deployedSha: null } }),
  )
  assert.ok(!v.some((x) => x.kind === 'clean-assay-unpublished'))
})

test('I5: run Complete but story not terminal is flagged', () => {
  const v = auditStoryConsistency(base({ storyStatus: 'Planned', run: { resultStatus: 'Complete' } }))
  assert.ok(v.some((x) => x.kind === 'terminal-disagreement'))
})

test('I5: story Complete but run Partial (not terminal) is flagged', () => {
  const v = auditStoryConsistency(base({ run: { resultStatus: 'Partial' } }))
  assert.ok(v.some((x) => x.kind === 'terminal-disagreement'))
})
