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

import { scanCandidateOwnDiff } from '../../lib/worker-workspace/candidate-secret-scan'
import { listCommitsToPublish } from '../../lib/worker-workspace/publish-range'
import { publishAcceptedCandidate } from '../../lib/worker-workspace/publish'

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

/** The diff reader the publish path uses: each commit against its own parent. */
const readDiff =
  (repo: string) =>
  async (commit: string): Promise<string | null> => {
    try {
      return await git(repo, ['diff', '--no-color', '--unified=0', `${commit}^`, commit])
    } catch {
      return null
    }
  }

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

test('refuses when the unpublished range cannot be read', async () => {
  const repo = await makeRepo()
  try {
    // An orphan root commit has no parent, so its own diff cannot be read. History exists (the base is
    // resolvable) but cannot be scanned, so the publisher must refuse rather than scan the tip and call
    // it complete.
    await git(repo.root, ['checkout', '-q', '--orphan', 'orphan'])
    const candidate = await commitFile(repo.root, 'solo.ts', 'export const solo = 1\n', 'orphan root')
    const outcome = await publishAcceptedCandidate({ repoRoot: repo.root, candidateCommit: candidate })
    assert.equal(outcome.outcome, 'publish-conflict')
    if (outcome.outcome !== 'publish-conflict') return
    assert.match(outcome.reason, /could not read/i)
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
