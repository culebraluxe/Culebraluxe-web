import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  candidateIterationsForGeneration,
  checkNoProgress,
  convergenceState,
  projectForgeConvergence,
  type ForgeCandidateEvent,
} from '../forge/forge-convergence'

// ENG-FORGE-CONVERGENCE-READMODEL-01 — candidate iteration + convergence read
// model (Scope B acceptance 8-14).

const A = 'a'.repeat(40)
const B = 'b'.repeat(40)
const C = 'c'.repeat(40)

function producer(sha: string, nodeId: string, at: string, runId?: string, gen = 0): ForgeCandidateEvent {
  return { kind: 'producer', generation: gen, sha, nodeId, runId: runId ?? null, at }
}
function qa(
  sha: string,
  verdict: 'PASS' | 'FAIL',
  at: string,
  o: { failureClass?: string | null; disposition?: string | null } = {},
  gen = 0,
): ForgeCandidateEvent {
  return {
    kind: 'qa',
    generation: gen,
    sha,
    runId: null,
    verdict,
    failureClass: o.failureClass ?? null,
    disposition: o.disposition ?? null,
    at,
  }
}

test('rule 8/9: a NEW SHA increments the iteration; a same-SHA run does not', () => {
  const its = candidateIterationsForGeneration([
    producer(A, 'smith', 't1'),
    producer(A, 'smith', 't2'), // no change -> no new iteration
    producer(B, 'smith', 't3'), // new SHA -> iteration 2
  ])
  assert.equal(its.length, 2)
  assert.deepEqual(its.map((i) => i.iteration), [1, 2])
  assert.deepEqual(its.map((i) => i.candidateSha), [A, B])
})

test('rule 4/10: QA binds to the exact candidate SHA; latest QA wins; no fabricated candidate', () => {
  const its = candidateIterationsForGeneration([
    producer(A, 'smith', 't1'),
    producer(B, 'smith', 't2'),
    qa(A, 'FAIL', 't3', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }),
    qa(A, 'PASS', 't4'), // retry same SHA -> same iteration, now PASS
    qa('Z', 'PASS', 't5'), // unknown sha -> ignored, no fabricated candidate
  ])
  assert.equal(its.length, 2)
  const itA = its.find((i) => i.candidateSha === A)!
  assert.equal(itA.iteration, 1)
  assert.equal(itA.qaVerdict, 'PASS')
  const itB = its.find((i) => i.candidateSha === B)!
  assert.equal(itB.qaVerdict, 'PENDING') // QA on Z did not touch B
  assert.equal(itB.iteration, 2)
})

test('rule 12/6: REPLAN-classified FAIL advances generation; numbering restarts but history is retained', () => {
  const execs = projectForgeConvergence({
    storyId: 'S1',
    processInstanceId: 'proc7',
    repairAttempts: 1,
    replanAttempts: 1,
    events: [
      producer(A, 'smith', 't1', null, 0),
      qa(A, 'FAIL', 't2', { failureClass: 'ARCHITECTURE_GAP', disposition: 'REPLAN' }, 0),
      producer(C, 'smith', 't3', null, 1),
      qa(C, 'PASS', 't4', {}, 1),
    ],
  })
  assert.equal(execs.length, 2)
  const e0 = execs.find((e) => e.generation === 0)!
  assert.equal(e0.convergenceState, 'REPLAN_REQUIRED')
  assert.equal(e0.iterations[0].iteration, 1)
  const e1 = execs.find((e) => e.generation === 1)!
  assert.equal(e1.convergenceState, 'CONVERGED')
  assert.equal(e1.executionId, 'proc7-e1')
  assert.equal(e1.iterations[0].iteration, 1) // numbering restarts per generation
})

test('rule 13: same SHA + same machine failure twice -> NO_PROGRESS HOLD (never auto re-repair)', () => {
  const np = checkNoProgress([
    producer(A, 'smith', 't1'),
    qa(A, 'FAIL', 't2', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }),
    qa(A, 'FAIL', 't3', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }), // same, no new candidate
  ])
  assert.equal(np.noProgress, true)
  assert.ok(np.reason?.includes('NO_PROGRESS'))
  const execs = projectForgeConvergence({
    storyId: 'S1',
    processInstanceId: 'proc7',
    repairAttempts: 2,
    replanAttempts: 0,
    events: [
      producer(A, 'smith', 't1'),
      qa(A, 'FAIL', 't2', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }),
      qa(A, 'FAIL', 't3', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }),
    ],
  })
  assert.equal(execs[0].convergenceState, 'HOLD')
  assert.ok(execs[0].holdReason?.includes('NO_PROGRESS'))
})

test('rule 13 (negative): an intervening NEW candidate clears the no-progress latch', () => {
  const np = checkNoProgress([
    producer(A, 'smith', 't1'),
    qa(A, 'FAIL', 't2', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }),
    producer(B, 'smith', 't3'),
    qa(B, 'FAIL', 't4', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }), // new candidate in between
  ])
  assert.equal(np.noProgress, false)
})

test('rule 13 (negative): a PASS between identical failures is machine progress, not a stuck loop', () => {
  const np = checkNoProgress([
    producer(A, 'smith', 't1'),
    qa(A, 'FAIL', 't2', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }),
    qa(A, 'PASS', 't3'), // machine recovered -> latch clears
    qa(A, 'FAIL', 't4', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }),
  ])
  assert.equal(np.noProgress, false)
})

test('rule 14: CONVERGED is derived ONLY from a PASS on the CURRENT candidate', () => {
  const pending = candidateIterationsForGeneration([producer(A, 'smith', 't1')])
  assert.equal(convergenceState(pending, { noProgress: false, reason: null }), 'ITERATING')
  const newer = candidateIterationsForGeneration([
    producer(A, 'smith', 't1'),
    qa(A, 'PASS', 't2'),
    producer(B, 'smith', 't3'),
    qa(B, 'FAIL', 't4', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }),
  ])
  assert.equal(convergenceState(newer, { noProgress: false, reason: null }), 'ITERATING')
})

test('rule 11: repair QA FAIL stays in the SAME generation/workspace and CONVERGES on the next SHA', () => {
  const execs = projectForgeConvergence({
    storyId: 'S1',
    processInstanceId: 'proc7',
    repairAttempts: 1,
    replanAttempts: 0,
    events: [
      producer(A, 'smith', 't1'),
      qa(A, 'FAIL', 't2', { failureClass: 'CODE_DEFECT', disposition: 'REPAIR' }),
      producer(B, 'smith', 't3'),
      qa(B, 'PASS', 't4'),
    ],
  })
  assert.equal(execs.length, 1) // one execution generation, not a replan
  assert.equal(execs[0].convergenceState, 'CONVERGED')
  assert.equal(execs[0].iterations.length, 2)
})

