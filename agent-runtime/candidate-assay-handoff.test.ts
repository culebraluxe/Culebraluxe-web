// ---------------------------------------------------------------------------
// ENG-FORGE-V4-10C — Exact Candidate Assay Handoff: focused tests.
//
// Reproduces the V4-11 false-positive path (Assay workspace provisioned from
// main@<head> while Smith's candidate commit was <candidate>; the failed
// packet command still normalized to Complete 100%) and proves every seam of
// the strict invariant fails closed. V6.1 inserts mandatory independent QA
// before deterministic Assay while preserving the exact candidate SHA.
// ---------------------------------------------------------------------------

import assert from 'node:assert/strict'
import test from 'node:test'

import {
  ASSAY_FAILURE_EVIDENCE,
  candidateVerifiedEvidenceLine,
  commitSha,
  finishedRunCandidateSha,
  isAssayTerminalRole,
  isCleanAssayEvidence,
  resolveAssayWorkspaceBase,
  smithCandidateSha,
  verifiedShaFromWorkspaceEvidence,
  workspaceEvidenceLine,
} from './candidate-assay-handoff'
import {
  normalizeAgentFinishForRole,
  type AssayFinishContext,
} from './repositories'
import { followFinishedLane } from './orchestrate-apply'
import { publishAcceptedCandidateAfterAssay } from './accepted-candidate-publish'
import type {
  PublishAcceptedCandidateInput,
  PublishAcceptedCandidateOutcome,
} from '../lib/worker-workspace'

const CANDIDATE = '549866555152c6f4bb55ffa9d45f19910ffab9f5'
const MAIN_BASE = '7b14c6b'.padEnd(40, '0')

const cleanInput = {
  resultStatus: 'Complete',
  completion: 100,
  notes: 'verifier finished',
  commitHash: null,
  testsSummary: 'candidate-assay-handoff.test.ts 18/18 pass',
}

function notesWithBase(baseSha: string, ref = baseSha): string {
  return workspaceEvidenceLine({
    branchName: 'agent/eng-forge-v4-11/626c756a-9bfd-46ad-9f6c-042de261f481',
    worktreePath: '/worktrees/eng-forge-v4-11-626c756a-9bfd-46ad-9f6c-042de261f481',
    baseRef: ref,
    baseCommit: baseSha,
  })
}

function context(candidateSha: string | null): AssayFinishContext {
  return { candidateSha }
}

test('isAssayTerminalRole covers reviewer + verifier only', () => {
  assert.equal(isAssayTerminalRole('reviewer'), true)
  assert.equal(isAssayTerminalRole('verifier'), true)
  assert.equal(isAssayTerminalRole('REVIEWER'), true)
  assert.equal(isAssayTerminalRole('builder'), false)
  assert.equal(isAssayTerminalRole('scout'), false)
  assert.equal(isAssayTerminalRole('architect'), false)
  assert.equal(isAssayTerminalRole(null), false)
  assert.equal(isAssayTerminalRole(undefined), false)
})

test('smithCandidateSha resolves the newest commit-bearing run; never invents one', () => {
  const C = CANDIDATE
  assert.equal(
    smithCandidateSha([
      { commitHash: null },
      { commitHash: C },
    ]),
    C,
  )
  assert.equal(smithCandidateSha([{ commitHash: C }, { commitHash: MAIN_BASE }]), C)
  assert.equal(smithCandidateSha([{ commitHash: null }]), null)
  assert.equal(smithCandidateSha([]), null)
  assert.equal(smithCandidateSha([{ commitHash: 'not-a-hash' }]), null)
})

test('finishedRunCandidateSha is strict: the just-finished run owns the candidate', () => {
  assert.equal(
    finishedRunCandidateSha([
      { commitHash: null },
      { commitHash: CANDIDATE },
    ]),
    null,
  )
  assert.equal(finishedRunCandidateSha([{ commitHash: CANDIDATE }]), CANDIDATE)
})

test('commitSha normalizes only real 40-hex hashes', () => {
  assert.equal(commitSha(CANDIDATE), CANDIDATE)
  assert.equal(commitSha(`  ${CANDIDATE.toUpperCase()}  `), CANDIDATE)
  assert.equal(commitSha('main'), null)
  assert.equal(commitSha(null), null)
  assert.equal(commitSha(''), null)
})

test('missing-file and non-zero command evidence is failure evidence', () => {
  assert.equal(ASSAY_FAILURE_EVIDENCE.test('all checks passed'), false)
  for (const dirty of [
    '1 failed, 11 passed',
    'agent-runtime/slack-notifier.test.ts does not exist in this checkout',
    'No test files found for slack-notifier.test.ts',
    'command not found: pnpm',
    'exit code 1 from the Assay command',
    'file is missing from the workspace',
    'candidate could not resolve to a commit',
    'verification policy gate rejected',
  ]) {
    assert.equal(ASSAY_FAILURE_EVIDENCE.test(dirty), true, dirty)
  }
})

test('isCleanAssayEvidence requires Complete and no failure marker', () => {
  assert.equal(
    isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary: '12 passed, 0 errors' }),
    true,
  )
  assert.equal(
    isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary: null }),
    true,
  )
  assert.equal(
    isCleanAssayEvidence({ resultStatus: 'Hold', testsSummary: 'clean summary' }),
    false,
  )
  assert.equal(
    isCleanAssayEvidence({ resultStatus: 'Complete', testsSummary: '1 failed' }),
    false,
  )
  assert.equal(
    isCleanAssayEvidence({
      resultStatus: 'Complete',
      testsSummary: 'slack-notifier.test.ts was not found; Assay command could not run',
    }),
    false,
  )
})

test('verifiedShaFromWorkspaceEvidence reads the base commit the Assay ran on', () => {
  assert.equal(
    verifiedShaFromWorkspaceEvidence(notesWithBase(CANDIDATE)),
    CANDIDATE,
  )
  assert.equal(
    verifiedShaFromWorkspaceEvidence(
      `Assay output above.\n\n${notesWithBase(MAIN_BASE, 'main')}`,
    ),
    MAIN_BASE,
  )
  assert.equal(verifiedShaFromWorkspaceEvidence('no workspace line'), null)
  assert.equal(verifiedShaFromWorkspaceEvidence(null), null)
})

test('candidateVerifiedEvidenceLine records the exact verified SHA', () => {
  assert.equal(
    candidateVerifiedEvidenceLine(CANDIDATE),
    `Assay verified candidate ${CANDIDATE} (workspace base == candidate).`,
  )
})

test('Assay lanes resolve their workspace base to the exact candidate commit', () => {
  for (const role of ['reviewer', 'verifier']) {
    const resolved = resolveAssayWorkspaceBase({
      role,
      candidateSha: CANDIDATE,
      fallbackBaseRef: 'main',
    })
    assert.deepEqual(resolved, { baseRef: CANDIDATE })
  }
})

test('non-Assay lanes keep the existing approved integration base', () => {
  for (const role of ['builder', 'scout', 'architect', null]) {
    const resolved = resolveAssayWorkspaceBase({
      role,
      candidateSha: CANDIDATE,
      fallbackBaseRef: 'main',
    })
    assert.deepEqual(resolved, { baseRef: 'main' })
  }
})

test('an Assay lane with no resolvable candidate fails closed (never main)', () => {
  for (const candidateSha of [null, '', '   ', 'not-a-commit']) {
    const resolved = resolveAssayWorkspaceBase({
      role: 'verifier',
      candidateSha,
      fallbackBaseRef: 'main',
    })
    assert.ok('error' in resolved, String(candidateSha))
    if ('error' in resolved) {
      assert.match(resolved.error, /refusing to provision from main/)
    }
  }
})

test('V4-11: verifier Assay provisioned from main cannot normalize to Complete', () => {
  const normalized = normalizeAgentFinishForRole(
    'verifier',
    {
      ...cleanInput,
      notes: notesWithBase(MAIN_BASE, 'main'),
    },
    context(CANDIDATE),
  )
  assert.equal(normalized.resultStatus, 'Hold')
  assert.notEqual(normalized.resultStatus, 'Complete')
  assert.equal(normalized.commitHash, null)
  assert.match(normalized.notes, /does not equal Smith candidate/)
  assert.match(normalized.notes, /549866555152/)
})

test('reviewer role gets the same wrong-base Hold semantics', () => {
  const normalized = normalizeAgentFinishForRole(
    'reviewer',
    {
      ...cleanInput,
      notes: notesWithBase(MAIN_BASE, 'main'),
    },
    context(CANDIDATE),
  )
  assert.equal(normalized.resultStatus, 'Hold')
  assert.match(normalized.notes, /does not equal Smith candidate/)
})

test('clean Assay that verified the exact candidate stays Complete + records the SHA', () => {
  const normalized = normalizeAgentFinishForRole(
    'verifier',
    {
      ...cleanInput,
      notes: notesWithBase(CANDIDATE),
    },
    context(CANDIDATE),
  )
  assert.equal(normalized.resultStatus, 'Complete')
  assert.equal(normalized.commitHash, null)
  assert.match(normalized.notes, /Assay verified candidate 549866555152/)
})

test('Assay with a clean summary but no candidate fails closed to Hold', () => {
  for (const role of ['reviewer', 'verifier']) {
    const normalized = normalizeAgentFinishForRole(
      role,
      {
        ...cleanInput,
        notes: notesWithBase(MAIN_BASE, 'main'),
      },
      context(null),
    )
    assert.equal(normalized.resultStatus, 'Hold', role)
    assert.match(normalized.notes, /no Smith candidate commit exists/)
  }
})

test('Assay with clean summary but no workspace evidence fails closed to Hold', () => {
  const normalized = normalizeAgentFinishForRole(
    'verifier',
    { ...cleanInput, notes: 'Assay ran but recorded no workspace line' },
    context(CANDIDATE),
  )
  assert.equal(normalized.resultStatus, 'Hold')
  assert.match(normalized.notes, /no workspace base evidence was recorded/)
})

test('verifier failed/missing command evidence is never Complete (no context either)', () => {
  for (const testsSummary of [
    '1 failed',
    'slack-notifier.test.ts does not exist in this checkout',
    'No test files found',
  ]) {
    const normalized = normalizeAgentFinishForRole('verifier', {
      ...cleanInput,
      testsSummary,
    })
    assert.equal(normalized.resultStatus, 'Hold', testsSummary)
    assert.equal(normalized.commitHash, null)
  }
})

test('clean verifier (legacy no-context caller) keeps Complete but never a commit', () => {
  const normalized = normalizeAgentFinishForRole('verifier', cleanInput)
  assert.equal(normalized.resultStatus, 'Complete')
  assert.equal(normalized.commitHash, null)
})

test('builder finish behavior is unchanged by the Assay invariants', () => {
  const builderInput = {
    resultStatus: 'Complete',
    completion: 100,
    notes: 'smith done',
    commitHash: CANDIDATE,
    testsSummary: 'all checks passed',
  }
  assert.deepEqual(normalizeAgentFinishForRole('builder', builderInput), builderInput)
})

const packetStory = {
  architectBrief: 'Implement the exact candidate Assay handoff.',
  acceptanceCriteria: '- Assay verifies the exact Smith candidate commit\n- never main',
  assayCommands: '- pnpm exec tsx --test agent-runtime/candidate-assay-handoff.test.ts',
}

test('builder finish WITH a candidate hands off to mandatory QA carrying the candidate', async () => {
  const enqueued: Array<{
    role: string
    specialInstructions: string | null
  }> = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V4-10C-WITH-CANDIDATE',
    finishedRole: 'builder',
    resultStatus: 'Complete',
    candidateSha: CANDIDATE,
    getStory: async () => packetStory,
    enqueue: async (input) => {
      enqueued.push({ role: input.role, specialInstructions: input.specialInstructions ?? null })
    },
    repoRoot: '/definitely/missing',
  })
  assert.equal(followed, 'inspector')
  assert.equal(enqueued.length, 1)
  assert.equal(enqueued[0]?.role, 'reviewer')
  assert.ok(enqueued[0]?.specialInstructions?.includes(CANDIDATE))
})

test('builder finish WITHOUT a candidate never launches QA or Assay', async () => {
  const enqueued: unknown[] = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V4-10C-NO-CANDIDATE',
    finishedRole: 'builder',
    resultStatus: 'Complete',
    candidateSha: null,
    getStory: async () => packetStory,
    enqueue: async (input) => {
      enqueued.push(input)
    },
    repoRoot: '/definitely/missing',
  })
  assert.equal(followed, null)
  assert.deepEqual(enqueued, [])
})

test('publish only runs when the Assay verified the exact candidate being published', async () => {
  let invoked = 0
  const publishSpy = async (
    input: PublishAcceptedCandidateInput,
  ): Promise<PublishAcceptedCandidateOutcome> => {
    invoked += 1
    const sha = input.candidateCommit ?? ''
    return {
      outcome: 'published',
      candidateCommit: sha,
      publishedMainHash: sha,
    }
  }

  const report = await publishAcceptedCandidateAfterAssay({
    role: 'verifier',
    resultStatus: 'Complete',
    testsSummary: 'all checks passed',
    candidateCommit: CANDIDATE,
    assayedCandidate: CANDIDATE,
    repoRoot: '/definitely/missing',
    publish: publishSpy,
  })
  assert.equal(report.action, 'published')
  assert.equal(invoked, 1)
})

test('publish refuses when the Assay verified a different base (V4-11 main case)', async () => {
  let invoked = 0
  const publishSpy = async () => {
    invoked += 1
    return { outcome: 'no-candidate' as const, reason: 'unexpected invocation' }
  }

  for (const assayedCandidate of [MAIN_BASE, null, '', '   ']) {
    const report = await publishAcceptedCandidateAfterAssay({
      role: 'verifier',
      resultStatus: 'Complete',
      testsSummary: 'all checks passed',
      candidateCommit: CANDIDATE,
      assayedCandidate,
      repoRoot: '/definitely/missing',
      publish: publishSpy,
    })
    assert.equal(report.action, 'not-eligible', String(assayedCandidate))
    assert.match(report.reason, /does not equal the publish candidate/)
  }
  assert.equal(invoked, 0, 'the git publish must never run on a verified-base mismatch')
})

test('omitting assayedCandidate keeps the legacy V4-10B publish decision unchanged', async () => {
  let invoked = 0
  const report = await publishAcceptedCandidateAfterAssay({
    role: 'reviewer',
    resultStatus: 'Complete',
    testsSummary: 'accepted-candidate-publish.test.ts 4/4 pass',
    candidateCommit: CANDIDATE,
    repoRoot: '/definitely/missing',
    publish: async (
      input: PublishAcceptedCandidateInput,
    ): Promise<PublishAcceptedCandidateOutcome> => {
      invoked += 1
      const sha = input.candidateCommit ?? ''
      return {
        outcome: 'published',
        candidateCommit: sha,
        publishedMainHash: sha,
      }
    },
  })
  assert.equal(report.action, 'published')
  assert.equal(invoked, 1)
})