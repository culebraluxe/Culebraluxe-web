import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  isTerminalAssayRole,
  publishAcceptedCandidateAfterAssay,
} from './accepted-candidate-publish'
import { publishAcceptedCandidate } from '../lib/worker-workspace'

function git(cwd: string, args: string[]): string {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}

function remoteHead(local: string): string {
  const out = git(local, ['ls-remote', 'origin', 'refs/heads/main'])
  return out.split(/\s+/)[0] ?? ''
}

/**
 * Local repo (main + `origin` = a bare remote) plus an optional candidate
 * branch `agent/<story>/run-1` holding a real Smith candidate commit ahead of
 * main. Mirrors the ENG-21 harness layout without touching the real repo.
 */
function repoPair(): { local: string; base: string } {
  const local = mkdtempSync(join(tmpdir(), 'forge-publish-local-'))
  const bare = mkdtempSync(join(tmpdir(), 'forge-publish-remote-'))
  git(local, ['init'])
  git(local, ['config', 'user.email', 'forge-publish-test@example.com'])
  git(local, ['config', 'user.name', 'Forge Publish Test'])
  writeFileSync(join(local, 'base.txt'), 'base\n')
  git(local, ['add', '.'])
  git(local, ['commit', '-m', 'base main'])
  git(local, ['branch', '-M', 'main'])
  git(local, ['init', '--bare', bare])
  git(local, ['remote', 'add', 'origin', bare])
  git(local, ['push', '-u', 'origin', 'main'])
  const base = git(local, ['rev-parse', 'HEAD'])
  return { local, base }
}

function makeCandidate(local: string, storyId = 'ENG-FORGE-TEST'): string {
  const branch = `agent/${storyId.toLowerCase()}/run-1`
  git(local, ['checkout', '-b', branch, 'main'])
  writeFileSync(join(local, 'candidate.txt'), 'candidate\n')
  git(local, ['add', '.'])
  git(local, ['commit', '-m', `${storyId}: accepted candidate change`])
  const sha = git(local, ['rev-parse', 'HEAD'])
  git(local, ['checkout', 'main'])
  return sha
}

test('clean Assay + real candidate + unchanged origin/main publishes automatically', async () => {
  const { local, base } = repoPair()
  const candidate = makeCandidate(local)

  const report = await publishAcceptedCandidateAfterAssay({
    role: 'reviewer',
    resultStatus: 'Complete',
    testsSummary: 'accepted-candidate-publish.test.ts 4/4 pass',
    candidateCommit: candidate,
    repoRoot: local,
  })

  assert.equal(report.action, 'published')
  if (report.action === 'published') {
    assert.equal(report.candidateCommit, candidate)
    assert.equal(report.publishedMainHash, candidate)
  }
  // origin/main now holds the accepted candidate; the local main ref is left
  // untouched and the candidate branch is preserved.
  assert.equal(remoteHead(local), candidate)
  assert.equal(git(local, ['rev-parse', 'main']), base)
  assert.equal(git(local, ['rev-parse', 'agent/eng-forge-test/run-1']), candidate)
})

test('verifier role (current assay lane binding) publishes the same happy path', async () => {
  const { local } = repoPair()
  const candidate = makeCandidate(local)

  const report = await publishAcceptedCandidateAfterAssay({
    role: 'verifier',
    resultStatus: 'Complete',
    testsSummary: 'all checks passed',
    candidateCommit: candidate,
    repoRoot: local,
  })

  assert.equal(report.action, 'published')
  assert.equal(remoteHead(local), candidate)
})

test('no candidate commit means no publication', async () => {
  const { local, base } = repoPair()
  makeCandidate(local)

  for (const candidateCommit of [null, undefined, '   ']) {
    const report = await publishAcceptedCandidateAfterAssay({
      role: 'reviewer',
      resultStatus: 'Complete',
      testsSummary: 'all checks passed',
      candidateCommit,
      repoRoot: local,
    })
    assert.equal(report.action, 'no-candidate', String(candidateCommit))
  }

  // The git primitive independently refuses an empty candidate.
  const primitive = await publishAcceptedCandidate({
    repoRoot: local,
    candidateCommit: null,
  })
  assert.equal(primitive.outcome, 'no-candidate')
  assert.equal(remoteHead(local), base)
})

test('failed/Hold Assay never publishes accepted code', async () => {
  const { local, base } = repoPair()
  const candidate = makeCandidate(local)
  let invoked = 0
  const spy = async () => {
    invoked += 1
    return { outcome: 'no-candidate' as const, reason: 'unexpected invocation' }
  }

  const dirtyResults = [
    { resultStatus: 'Complete', testsSummary: '1 failed, 11 passed' },
    { resultStatus: 'Complete', testsSummary: 'verification violation detected' },
    { resultStatus: 'Complete', testsSummary: 'policy gate rejected the change' },
    { resultStatus: 'Hold', testsSummary: 'clean summary but not complete' },
    { resultStatus: 'Assay Failed', testsSummary: 'tests failed' },
    { resultStatus: 'Partial', testsSummary: '2 passed, 1 partial' },
  ]

  for (const dirty of dirtyResults) {
    const report = await publishAcceptedCandidateAfterAssay({
      role: 'reviewer',
      resultStatus: dirty.resultStatus,
      testsSummary: dirty.testsSummary,
      candidateCommit: candidate,
      repoRoot: local,
      publish: spy,
    })
    assert.equal(report.action, 'not-eligible', JSON.stringify(dirty))
  }

  assert.equal(invoked, 0, 'the git publish must never run for a failed Assay')
  assert.equal(remoteHead(local), base)
})

test('remote-main divergence fails closed: no force-push, no discard, conflict reported', async () => {
  const { local, base } = repoPair()
  const candidateBranch = 'agent/eng-forge-test/run-1'
  const candidate = makeCandidate(local)

  // Remote main advances past the candidate's base before publication runs.
  writeFileSync(join(local, 'other-story.txt'), 'other\n')
  git(local, ['add', '.'])
  git(local, ['commit', '-m', 'another accepted story already on main'])
  const advanced = git(local, ['rev-parse', 'HEAD'])
  git(local, ['push', 'origin', 'main'])
  assert.notEqual(advanced, base)

  const report = await publishAcceptedCandidateAfterAssay({
    role: 'reviewer',
    resultStatus: 'Complete',
    testsSummary: 'all checks passed',
    candidateCommit: candidate,
    repoRoot: local,
  })

  assert.equal(report.action, 'publish-conflict')
  if (report.action === 'publish-conflict') {
    assert.equal(report.remoteMainHash, advanced)
    assert.equal(report.candidateCommit, candidate)
    assert.match(report.reason, /not an ancestor/i)
  }
  // main was NOT rewound to the candidate and the candidate commit survives.
  assert.equal(remoteHead(local), advanced)
  assert.equal(git(local, ['rev-parse', candidateBranch]), candidate)
})

test('publication is gated to Assay/verification lanes only', async () => {
  const { local, base } = repoPair()
  const candidate = makeCandidate(local)
  let invoked = 0
  const spy = async () => {
    invoked += 1
    return { outcome: 'no-candidate' as const, reason: 'unexpected invocation' }
  }

  for (const role of ['builder', 'scout', 'architect', null]) {
    const report = await publishAcceptedCandidateAfterAssay({
      role,
      resultStatus: 'Complete',
      testsSummary: 'all checks passed',
      candidateCommit: candidate,
      repoRoot: local,
      publish: spy,
    })
    assert.equal(report.action, 'not-eligible', String(role))
  }
  assert.equal(invoked, 0)
  assert.equal(remoteHead(local), base)
  assert.equal(isTerminalAssayRole('reviewer'), true)
  assert.equal(isTerminalAssayRole('verifier'), true)
  assert.equal(isTerminalAssayRole('builder'), false)
})

test('an unresolvable recorded candidate is a factual conflict, never a silent publish', async () => {
  const { local, base } = repoPair()
  const missing = 'f'.repeat(40)

  const report = await publishAcceptedCandidateAfterAssay({
    role: 'reviewer',
    resultStatus: 'Complete',
    testsSummary: 'all checks passed',
    candidateCommit: missing,
    repoRoot: local,
  })

  assert.equal(report.action, 'publish-conflict')
  if (report.action === 'publish-conflict') {
    assert.match(report.reason, /not present in the local repository/)
  }
  assert.equal(remoteHead(local), base)
})

test('publishing an already-published candidate is idempotent success', async () => {
  const { local } = repoPair()
  const candidate = makeCandidate(local)
  // First publish succeeds...
  const first = await publishAcceptedCandidateAfterAssay({
    role: 'reviewer',
    resultStatus: 'Complete',
    testsSummary: 'all checks passed',
    candidateCommit: candidate,
    repoRoot: local,
  })
  assert.equal(first.action, 'published')
  // ...and a retry reports the same factual outcome without rewriting main.
  const retry = await publishAcceptedCandidateAfterAssay({
    role: 'reviewer',
    resultStatus: 'Complete',
    testsSummary: 'all checks passed',
    candidateCommit: candidate,
    repoRoot: local,
  })
  assert.equal(retry.action, 'published')
  assert.equal(remoteHead(local), candidate)
})
