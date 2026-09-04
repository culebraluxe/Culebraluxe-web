import assert from 'node:assert/strict'
import test from 'node:test'

import {
  isCleanAssayEvidence,
  workspaceEvidenceLine,
} from './candidate-assay-handoff'
import { assayFailureEvidence } from './orchestrate-apply'
import { normalizeAgentFinishForRole } from './repositories'

const CANDIDATE = '82749b75d3aa07aa8943a6a6a1250f8b922e9170'

function exactCandidateNotes(): string {
  return workspaceEvidenceLine({
    branchName: 'agent/eng-forge-v5-11/regression',
    worktreePath: '/worktrees/eng-forge-v5-11-regression',
    baseRef: CANDIDATE,
    baseCommit: CANDIDATE,
  })
}

test('zero failure counters are success evidence, not Assay failure markers', () => {
  for (const testsSummary of [
    '25/25 pass, 0 fail',
    'tests 26, pass 26, fail 0, cancelled 0, skipped 0',
    '12 passed, 0 failed',
    'errors: 0; 9 passed',
    '0 failures; all checks passed',
  ]) {
    assert.equal(
      isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary }),
      true,
      testsSummary,
    )
  }
})

test('positive or ambiguous failure evidence still fails closed', () => {
  for (const testsSummary of [
    '25 pass, 1 fail',
    '1 failed, 24 passed',
    'failure in exact-candidate check',
    'policy violation',
    'exit code 1',
    'command not found: pnpm',
  ]) {
    assert.equal(
      isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary }),
      false,
      testsSummary,
    )
  }
})

test('V5-11-shaped exact-candidate Assay with 0 fail remains Complete', () => {
  const normalized = normalizeAgentFinishForRole(
    'verifier',
    {
      resultStatus: 'Complete',
      completion: 100,
      notes: exactCandidateNotes(),
      commitHash: null,
      testsSummary:
        '`pnpm exec tsx --test agent-runtime/**/*team*.test.ts agent-runtime/**/*routing*.test.ts` → exit 0; 25/25 pass, 0 fail',
    },
    { candidateSha: CANDIDATE },
  )

  assert.equal(normalized.resultStatus, 'Complete')
  assert.equal(normalized.commitHash, null)
  assert.match(normalized.notes, /Assay verified candidate 82749b75d3aa/)
})

test('configured Assay commands are never fabricated as failed commands', () => {
  const evidence = assayFailureEvidence({
    testsSummary: 'verification result was not Complete',
    failedCommands: [
      'pnpm exec tsx --test agent-runtime/**/*team*.test.ts agent-runtime/**/*routing*.test.ts',
    ],
  })

  assert.equal(
    evidence,
    'verification result was not Complete | assay commands: pnpm exec tsx --test agent-runtime/**/*team*.test.ts agent-runtime/**/*routing*.test.ts',
  )
  assert.doesNotMatch(evidence ?? '', /failed commands:/i)
})
