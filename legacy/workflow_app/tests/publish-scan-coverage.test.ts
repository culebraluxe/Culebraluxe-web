// ---------------------------------------------------------------------------
// FORGE-PUBLISH-SCAN-COVERAGE-01 — the credential scan covers every commit being published.
//
// The publisher used to hand the scanner `commits: [candidate]` — the tip only — so a credential
// introduced in an EARLIER unpublished commit rode along behind a clean final commit (Astra review 1.3:
// a tip-only scan found 0 findings where scanning both unpublished commits found 1).
//
// This fence drives the PUBLISHER end-to-end against real temporary git repositories, so a regression to
// the tip-only shape fails here. The planted value is assembled at runtime so this file never carries a
// credential-shaped literal of its own.
//
// The test names are the acceptance assertions the contract maps to, printed on node:test's pass/fail
// marker lines:
//   blocks a credential in an earlier unpublished commit behind a clean tip
//   refuses when the unpublished range cannot be read
//   excludes commits already on the remote
//   names the introducing commit without printing the credential value
//   drives both the blocked and the excluded cases
// ---------------------------------------------------------------------------

import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'
import test from 'node:test'

import { commitOwnDiffReader, scanCandidateOwnDiff } from '@/lib/worker-workspace/candidate-secret-scan'
import { listCommitsToPublish } from '@/lib/worker-workspace/publish-range'
import { publishAcceptedCandidate } from '@/lib/worker-workspace/publish'

const execFileAsync = promisify(execFile)

/** A value the catalog calls an `openai-style key`, built at runtime (no literal in this file). */
const PLANTED_CREDENTIAL = `sk-${'a'.repeat(20)}`

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync(
    'git',
    ['-c', 'user.name=Fence', '-c', 'user.email=fence@example.test', '-c', 'commit.gpgsign=false', ...args],
    { cwd, encoding: 'utf8' },
  )
  return stdout.trim()
}

async function commitFile(
  root: string,
  name: string,
  content: string,
  message: string,
): Promise<string> {
  await writeFile(join(root, name), content, 'utf8')
  await git(root, ['add', '-A'])
  await git(root, ['commit', '-q', '-m', message])
  return git(root, ['rev-parse', 'HEAD'])
}

type Repo = { root: string; origin: string; base: string; cleanup: () => Promise<void> }

/** A working clone with `origin` and a base commit pushed to `origin/main` (a tracking ref exists). */
async function makeRepo(opts?: { baseCarriesCredential?: boolean }): Promise<Repo> {
  const dir = await mkdtemp(join(tmpdir(), 'publish-scan-coverage-'))
  const origin = join(dir, 'origin.git')
  const root = join(dir, 'work')
  await git(dir, ['init', '--bare', '-q', origin])
  await mkdir(root)
  await git(root, ['init', '-q', '-b', 'main'])
  const baseContent = opts?.baseCarriesCredential
    ? `export const legacy = "${PLANTED_CREDENTIAL}"\n`
    : 'export const base = 1\n'
  const base = await commitFile(root, opts?.baseCarriesCredential ? 'legacy.ts' : 'base.ts', baseContent, 'base')
  await git(root, ['remote', 'add', 'origin', origin])
  await git(root, ['push', '-q', 'origin', 'main'])
  return {
    root,
    origin,
    base,
    cleanup: async () => {
      await rm(dir, { recursive: true, force: true })
    },
  }
}

/** The diff reader the publish path uses — the PRODUCTION one, so root commits are read the same way here. */
const readDiff = (repo: string) => commitOwnDiffReader(repo)

/** Blocked case: credential in the FIRST of two unpublished commits, clean tip. */
async function blockedCase(): Promise<{
  outcome: Awaited<ReturnType<typeof publishAcceptedCandidate>>
  remoteHead: string
  base: string
  first: string
  second: string
  range: Awaited<ReturnType<typeof listCommitsToPublish>>
  fullFindings: number
  tipOnlyFindings: number
}> {
  const repo = await makeRepo()
  try {
    await git(repo.root, ['checkout', '-q', '-b', 'agent/publish-scan'])
    const first = await commitFile(
      repo.root,
      'first.ts',
      `const k = "${PLANTED_CREDENTIAL}"\n`,
      'first: introduces a key',
    )
    const second = await commitFile(repo.root, 'second.ts', 'export const second = 2\n', 'second: clean')
    const candidate = await git(repo.root, ['rev-parse', 'HEAD'])
    const range = await listCommitsToPublish({
      repoRoot: repo.root,
      candidate,
      remoteName: 'origin',
      remoteBranch: 'main',
    })
    const full = await scanCandidateOwnDiff({ commits: range.commits, readDiff: readDiff(repo.root) })
    const tipOnly = await scanCandidateOwnDiff({ commits: [candidate], readDiff: readDiff(repo.root) })
    const outcome = await publishAcceptedCandidate({ repoRoot: repo.root, candidateCommit: candidate })
    const remoteHead = (await git(repo.root, ['ls-remote', 'origin', 'refs/heads/main'])).split(/\s+/)[0]
    return {
      outcome,
      remoteHead,
      base: repo.base,
      first,
      second,
      range,
      fullFindings: full.findings.length,
      tipOnlyFindings: tipOnly.findings.length,
    }
  } finally {
    await repo.cleanup()
  }
}

/** Excluded case: a credential already on the remote (in the base) must not refuse a clean lane. */
async function excludedCase(): Promise<Awaited<ReturnType<typeof publishAcceptedCandidate>>> {
  const repo = await makeRepo({ baseCarriesCredential: true })
  try {
    await git(repo.root, ['checkout', '-q', '-b', 'agent/clean-lane'])
    await commitFile(repo.root, 'clean.ts', 'export const ok = true\n', 'clean lane work')
    const candidate = await git(repo.root, ['rev-parse', 'HEAD'])
    return await publishAcceptedCandidate({ repoRoot: repo.root, candidateCommit: candidate })
  } finally {
    await repo.cleanup()
  }
}

test('blocks a credential in an earlier unpublished commit behind a clean tip', async () => {
  const result = await blockedCase()
  assert.equal(result.range.commits.length, 2, 'both unpublished commits are in the push range')
  assert.equal(result.range.commits[0], result.second, 'the tip is first')
  assert.equal(result.fullFindings, 1, 'the range scan finds the credential in the earlier commit')
  assert.equal(result.tipOnlyFindings, 0, 'the old tip-only scan is blind to it — the defect this closes')
  assert.equal(result.outcome.outcome, 'candidate-secret', 'publication is refused')
  assert.equal(result.remoteHead, result.base, 'nothing reached the remote')
})

test('a ROOT commit is scanned against the empty tree, so a credential in it is found', async () => {
  const repo = await makeRepo()
  try {
    // An orphan root commit has no parent, so `git diff <commit>^ <commit>` cannot read it. A root commit
    // introduces its WHOLE tree, which is expressible as a diff against the empty tree — so the history is
    // read rather than declared unreadable, and a credential in it is found (work package C, item 6).
    await git(repo.root, ['checkout', '-q', '--orphan', 'orphan'])
    const candidate = await commitFile(
      repo.root,
      'solo.ts',
      `export const solo = "${PLANTED_CREDENTIAL}"\n`,
      'orphan root with a key',
    )
    const range = await listCommitsToPublish({
      repoRoot: repo.root,
      candidate,
      remoteName: 'origin',
      remoteBranch: 'main',
    })
    const scan = await scanCandidateOwnDiff({ commits: range.commits, readDiff: readDiff(repo.root) })
    assert.deepEqual(scan.unreadable, [], 'a root commit is readable against the empty tree')
    assert.equal(scan.findings.length, 1, 'the credential the root commit introduces is found')
    const outcome = await publishAcceptedCandidate({ repoRoot: repo.root, candidateCommit: candidate })
    assert.equal(outcome.outcome, 'candidate-secret')
    const remoteHead = (await git(repo.root, ['ls-remote', 'origin', 'refs/heads/main'])).split(/\s+/)[0]
    assert.equal(remoteHead, repo.base, 'nothing reached the remote')
  } finally {
    await repo.cleanup()
  }
})

test('excludes commits already on the remote', async () => {
  const outcome = await excludedCase()
  assert.equal(outcome.outcome, 'published', 'a credential already on the remote does not refuse a clean lane')
})

// ---------------------------------------------------------------------------
// WORK PACKAGE C — THE RANGE COMES FROM THE DESTINATION, NOT FROM A LOCAL REF.
//
// Astra reproduced: local main pointing at the candidate with NO `origin/main` tracking ref produced an EMPTY
// range, so only the tip was scanned while two unpublished commits existed. The same reasoning covers a STALE
// tracking ref — it says what the remote had when this clone last looked — so neither local main nor the
// tracking cache may decide the boundary. The destination is asked directly.
// ---------------------------------------------------------------------------

/** Two unpublished commits with a credential in the first, arranged so no local ref can help. */
async function unpublishedBehindLocalMain(mode: 'no-tracking' | 'stale-tracking') {
  const repo = await makeRepo()
  await git(repo.root, ['checkout', '-q', '-b', 'agent/range'])
  const first = await commitFile(repo.root, 'first.ts', `const k = "${PLANTED_CREDENTIAL}"\n`, 'first: key')
  const second = await commitFile(repo.root, 'second.ts', 'export const second = 2\n', 'second: clean')
  const candidate = await git(repo.root, ['rev-parse', 'HEAD'])
  // LOCAL main MOVES TO THE CANDIDATE: the false story "main already contains everything".
  await git(repo.root, ['branch', '-f', 'main', candidate])
  if (mode === 'no-tracking') {
    await git(repo.root, ['update-ref', '-d', 'refs/remotes/origin/main'])
  } else {
    // A STALE tracking ref claiming the remote is already at the candidate: the cache says "nothing to send".
    await git(repo.root, ['update-ref', 'refs/remotes/origin/main', candidate])
  }
  try {
    const range = await listCommitsToPublish({
      repoRoot: repo.root,
      candidate,
      remoteName: 'origin',
      remoteBranch: 'main',
    })
    const scan = await scanCandidateOwnDiff({ commits: range.commits, readDiff: readDiff(repo.root) })
    const outcome = await publishAcceptedCandidate({ repoRoot: repo.root, candidateCommit: candidate })
    const remoteHead = (await git(repo.root, ['ls-remote', 'origin', 'refs/heads/main'])).split(/\s+/)[0]
    return { range, scan, outcome, remoteHead, base: repo.base, first, second, candidate }
  } finally {
    await repo.cleanup()
  }
}

test('with no origin/main tracking ref, the earlier unpublished commit is STILL scanned', async () => {
  const result = await unpublishedBehindLocalMain('no-tracking')
  assert.equal(result.range.source, 'remote-ref', 'the boundary came from the destination, not from local main')
  assert.equal(result.range.base, result.base)
  assert.equal(result.range.commits.length, 2, 'the earlier unpublished commit is in the range')
  assert.equal(result.scan.findings.length, 1, 'and the credential in it is found')
  assert.equal(result.outcome.outcome, 'candidate-secret', 'publication is refused')
  assert.equal(result.remoteHead, result.base, 'nothing reached the remote')
})

test('a STALE tracking ref cannot omit a commit that is actually being published', async () => {
  const result = await unpublishedBehindLocalMain('stale-tracking')
  assert.equal(result.range.source, 'remote-ref')
  assert.equal(result.range.commits.length, 2, 'the stale cache would have said zero')
  assert.equal(result.scan.findings.length, 1)
  assert.equal(result.outcome.outcome, 'candidate-secret')
  assert.equal(result.remoteHead, result.base)
})

test('a destination branch that does not exist is scanned as its WHOLE history, root commit included', async () => {
  const repo = await makeRepo({ baseCarriesCredential: true })
  try {
    // A branch the remote has never had. The remote answers authoritatively that it does not exist (a bare
    // remote refuses to delete its CURRENT branch, so the destination is a new name rather than a deleted one).
    const candidate = await git(repo.root, ['rev-parse', 'HEAD'])
    const range = await listCommitsToPublish({
      repoRoot: repo.root,
      candidate,
      remoteName: 'origin',
      remoteBranch: 'release-never-existed',
    })
    assert.equal(range.source, 'new-branch')
    assert.equal(range.unreadable, false, 'a missing branch is an answer, not an unreadable remote')
    assert.equal(range.fullHistory, true)
    assert.ok(range.commits.includes(repo.base), 'the ROOT commit is in the range')
    const scan = await scanCandidateOwnDiff({ commits: range.commits, readDiff: readDiff(repo.root) })
    assert.equal(scan.findings.length, 1, 'the credential in the root commit is found')
    const outcome = await publishAcceptedCandidate({
      repoRoot: repo.root,
      candidateCommit: candidate,
      remoteBranch: 'release-never-existed',
    })
    assert.equal(outcome.outcome, 'candidate-secret', 'publication is refused')
  } finally {
    await repo.cleanup()
  }
})

test('an UNREADABLE destination refuses rather than reporting a clean tip-only scan', async () => {
  const repo = await makeRepo()
  try {
    await commitFile(repo.root, 'clean.ts', 'export const ok = 1\n', 'clean')
    const candidate = await git(repo.root, ['rev-parse', 'HEAD'])
    const range = await listCommitsToPublish({
      repoRoot: repo.root,
      candidate,
      remoteName: 'origin-does-not-exist',
      remoteBranch: 'main',
    })
    assert.equal(range.unreadable, true)
    assert.equal(range.source, 'unreadable')
    assert.match(String(range.refusal), /could not be read/)
    // And the publisher refuses for the same reason, naming the state rather than scanning one commit.
    const outcome = await publishAcceptedCandidate({
      repoRoot: repo.root,
      candidateCommit: candidate,
      remoteName: 'origin-does-not-exist',
    })
    assert.equal(outcome.outcome, 'publish-conflict')
    if (outcome.outcome !== 'publish-conflict') return
    assert.match(outcome.reason, /could not establish the commits to publish/)
  } finally {
    await repo.cleanup()
  }
})

test('names the introducing commit without printing the credential value', async () => {
  const result = await blockedCase()
  assert.equal(result.outcome.outcome, 'candidate-secret')
  if (result.outcome.outcome !== 'candidate-secret') return
  const finding = result.outcome.findings[0]
  assert.equal(finding?.rule, 'openai-style key')
  assert.equal(finding?.file, 'first.ts')
  assert.equal(finding?.commit, result.first, 'the finding names the commit that introduced the line')
  assert.match(result.outcome.reason, /openai-style key/)
  assert.match(result.outcome.reason, new RegExp(result.first.slice(0, 12)))
  assert.doesNotMatch(result.outcome.reason, new RegExp(PLANTED_CREDENTIAL), 'the value is never printed')
})

test('drives both the blocked and the excluded cases', async () => {
  const blocked = await blockedCase()
  const excluded = await excludedCase()
  assert.equal(blocked.outcome.outcome, 'candidate-secret')
  assert.equal(excluded.outcome, 'published')
})
