import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'

import {
  FIRST_VIOLATIONS,
  classifyFirstViolation,
  renderFirstViolation,
} from '@/legacy/workflow_app/forge/first-violation'

// ---------------------------------------------------------------------------
// WHICH DOOR FAILED FIRST (AgentRx, arXiv 2602.02475).
//
// When a long agent dies at step 42, engineers debug step 42 — but the unrecoverable cut was
// earlier, and everything after it is drift. Forge can stop a generation that looped too long
// (the MAP cap); this decides WHAT to call the cause, from evidence, with 'unknown' preferred
// over a guess.
// ---------------------------------------------------------------------------

test('first viol: an acceptance that was never pinned down is UNDESPECIFIED', () => {
  const verdict = classifyFirstViolation({ acceptanceIncomplete: true, repeatedCandidateFailure: true })
  assert.equal(verdict.firstViol, 'underspecified')
  assert.match(verdict.because.join(' '), /acceptance or ownership was incomplete/)
})

test('first viol: a repeated candidate with a refused door is a SYSTEM fault', () => {
  const verdict = classifyFirstViolation({
    repeatedCandidateFailure: true,
    doorRefused: true,
    reasons: ['MODEL TURN CAP: this generation has already dispatched 10 turns'],
  })
  assert.equal(verdict.firstViol, 'system')
  assert.match(verdict.because.join(' '), /same candidate re-failed/)
  assert.match(verdict.because.join(' '), /observed: MODEL TURN CAP/)
})

test('first viol: evidence that could not be written is a SYSTEM fault', () => {
  const verdict = classifyFirstViolation({ evidenceWriteFailed: true })
  assert.equal(verdict.firstViol, 'system')
})

test('first viol: no observations is UNKNOWN, never a fabricated cause', () => {
  const verdict = classifyFirstViolation({})
  assert.equal(verdict.firstViol, 'unknown')
  assert.match(verdict.because.join(' '), /no door, candidate repeat or acceptance gap/)
})

test('first viol: the rendered line names the cause and its evidence', () => {
  const line = renderFirstViolation(classifyFirstViolation({ doorRefused: true }))
  assert.match(line, /^FIRST_VIOL=system — /)
  assert.match(line, /a door refused the lane/)
})

test('first viol: the vocabulary stays small enough to be actionable', () => {
  assert.deepEqual([...FIRST_VIOLATIONS], ['system', 'underspecified', 'unknown'])
})

// --- the wiring fence -------------------------------------------------------

const RUNNER = readFileSync(
  new URL('../forge/agent-runtime-role-runner.ts', import.meta.url),
  'utf8',
)

test('first viol: a turn-cap trip classifies the cause, records it, and says it in the error', () => {
  const capAt = RUNNER.indexOf('if (!turnBudget.allowed) {')
  const classifyAt = RUNNER.indexOf('classifyFirstViolation({')
  const factsAt = RUNNER.indexOf('readForgeGenerationFacts(resolvedStory.id)')
  const recordAt = RUNNER.indexOf('recordForgeFirstViolation(resolvedStory.id')
  const throwAt = RUNNER.indexOf('${turnBudget.reason} ${line}')

  assert.ok(capAt > 0, 'the cap must be enforced')
  assert.ok(factsAt > capAt, 'the facts come from the generation run rows')
  assert.ok(classifyAt > factsAt, 'classification uses those facts')
  assert.ok(recordAt > classifyAt, 'the label is recorded after it is decided')
  assert.ok(throwAt > recordAt, 'the operator sees the cause in the failure they read')
  assert.match(
    RUNNER,
    /acceptanceIncomplete: leadRoutingContext\.allowedProofs\.length === 0/,
    'no frozen proof is an underspecification, and the code says so',
  )
  assert.match(RUNNER, /operation|'forge\.first-violation'/, 'a recording failure is captured, not swallowed')
})
