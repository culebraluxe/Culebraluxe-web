import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  projectStoryRunsToConvergence,
  type ForgeNodeRun,
} from '../forge/forge-convergence-projector'

// Scope D data glue: durable role-node runs -> convergence executions.

const A = 'a'.repeat(40)
const C = 'c'.repeat(40)

function run(nodeId: string, commitHash: string | null, resultStatus: string | null, at: string): ForgeNodeRun {
  return { nodeId, storyRunId: `run_${at}`, commitHash, createdAt: at, resultStatus }
}

test('convergence projector: replan advances generation; numbering restarts but history is kept', () => {
  const execs = projectStoryRunsToConvergence({
    storyId: 'S1',
    processInstanceId: 'proc7',
    repairAttempts: 1,
    replanAttempts: 1,
    runs: [
      run('smith', A, 'Complete', 't1'),
      run('qa_verify', A, 'fail', 't2'),
      run('repair_architect', null, null, 't3'), // replan -> generation 1
      run('smith', C, 'Complete', 't4'),
      run('qa_verify', C, 'pass', 't5'),
    ],
  })
  assert.equal(execs.length, 2)
  const e0 = execs.find((e) => e.generation === 0)!
  const e1 = execs.find((e) => e.generation === 1)!
  assert.equal(e0.executionId, 'proc7-e0')
  assert.equal(e0.iterations[0].candidateSha, A)
  assert.equal(e0.iterations[0].qaVerdict, 'FAIL')
  assert.equal(e1.executionId, 'proc7-e1')
  assert.equal(e1.iterations[0].iteration, 1) // numbering restarts per generation
  assert.equal(e1.convergenceState, 'CONVERGED')
})

test('convergence projector: same SHA re-fails without a new candidate -> NO_PROGRESS HOLD', () => {
  const execs = projectStoryRunsToConvergence({
    storyId: 'S1',
    processInstanceId: 'proc7',
    repairAttempts: 2,
    replanAttempts: 0,
    runs: [
      run('smith', A, 'Complete', 't1'),
      run('qa_verify', A, 'fail', 't2'),
      run('qa_verify', A, 'fail', 't3'),
    ],
  })
  assert.equal(execs[0].convergenceState, 'HOLD')
  assert.ok(execs[0].holdReason?.includes('NO_PROGRESS'))
})

test('convergence projector: a clean QA PASS after repair converges in the same generation', () => {
  const execs = projectStoryRunsToConvergence({
    storyId: 'S1',
    processInstanceId: 'proc7',
    repairAttempts: 1,
    replanAttempts: 0,
    runs: [
      run('smith', A, 'Complete', 't1'),
      run('qa_verify', A, 'fail', 't2'),
      run('repair_smith', C, 'Complete', 't3'),
      run('qa_verify', C, 'pass', 't4'),
    ],
  })
  assert.equal(execs.length, 1)
  assert.equal(execs[0].convergenceState, 'CONVERGED')
  assert.equal(execs[0].iterations.length, 2)
})
