import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { promisify } from 'node:util'

import { scanCandidateOwnDiff } from '../../lib/worker-workspace/candidate-secret-scan'
import { listCommitsToPublish } from '../../lib/worker-workspace/publish-range'

const execFileAsync = promisify(execFile)

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

async function makeRepo(): Promise<string> {
  const repo = await mkdtemp(join(tmpdir(), 'forge-publish-range-'))
  await git(repo, ['init', '-b', 'main', '-q'])
  await git(repo, ['config', 'user.name', 'fence'])
  await git(repo, ['config', 'user.email', 'fence@test'])
  await git(repo, ['config', 'commit.gpgsign', 'false'])
  await writeFile(join(repo, 'base.ts'), 'export const base = 1\n')
  await git(repo, ['add', '-A'])
  await git(repo, ['commit', '-q', '-m', 'base'])
  return repo
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

// ---------------------------------------------------------------------------
// FORGE-PUBLISH-SCAN-COVERAGE-01 — the scan covers the whole push, not the tip.
//
// Astra review 1.3, reproduced against the reviewed sha: the publisher supplied `commits: [candidate]`
// and the reader diffs `<c>^..<c>`, so only the final commit was ever read. His synthetic case: a tip-only
// scan found 0 findings where scanning both unpublished commits found 1. A credential introduced in an
// earlier commit rode along behind a clean tip. This fence rebuilds that case and inverts the outcome.
// ---------------------------------------------------------------------------

test('publish scan: a credential in an EARLIER unpublished commit is found, and the tip-only scan missed it', async () => {
  const repo = await makeRepo()
  try {
    // A lane commits on its OWN branch, which is the shape the range is measured in: main stays at the
    // base and both lane commits are unpublished.
    await git(repo, ['checkout', '-q', '-b', 'agent/publish-scan/e1'])
    // Commit one of two: carries a credential-shaped value in the shape the catalog matches (the same
    // shape the scanner's own test plants).
    await writeFile(join(repo, 'first.ts'), `const k = "sk-${'a'.repeat(20)}"\n`)
    await git(repo, ['add', '-A'])
    await git(repo, ['commit', '-q', '-m', 'first: introduces a key'])
    // Commit two: unrelated, and clean.
    await writeFile(join(repo, 'second.ts'), 'export const second = 2\n')
    await git(repo, ['add', '-A'])
    await git(repo, ['commit', '-q', '-m', 'second: unrelated'])
    const candidate = await git(repo, ['rev-parse', 'HEAD'])

    const range = await listCommitsToPublish({
      repoRoot: repo,
      candidate,
      remoteName: 'origin',
      remoteBranch: 'main',
    })
    assert.equal(range.commits.length, 2, 'both unpublished commits are in the range')
    assert.equal(range.commits[0], candidate, 'the tip is first')
    assert.equal(range.source, 'local-base', 'measured from the local base when no tracking ref exists')

    const full = await scanCandidateOwnDiff({ commits: range.commits, readDiff: readDiff(repo) })
    assert.equal(full.findings.length, 1, 'the range scan finds the credential in the earlier commit')

    const tipOnly = await scanCandidateOwnDiff({ commits: [candidate], readDiff: readDiff(repo) })
    assert.equal(tipOnly.findings.length, 0, 'and the old tip-only scan is blind to it — the defect this closes')
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('publish scan: history already on the remote is excluded, so a lane is never refused for a foreign commit', async () => {
  const repo = await makeRepo()
  try {
    const base = await git(repo, ['rev-parse', 'HEAD'])
    await git(repo, ['update-ref', 'refs/remotes/origin/main', base])
    await writeFile(join(repo, 'lane.ts'), 'export const lane = 1\n')
    await git(repo, ['add', '-A'])
    await git(repo, ['commit', '-q', '-m', 'lane work'])
    const candidate = await git(repo, ['rev-parse', 'HEAD'])

    const range = await listCommitsToPublish({
      repoRoot: repo,
      candidate,
      remoteName: 'origin',
      remoteBranch: 'main',
    })
    assert.equal(range.source, 'remote-tracking', 'the tracking ref is preferred when it exists')
    assert.equal(range.base, base)
    assert.deepEqual(range.commits, [candidate], 'only the lane own commit is in the range')
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('publish scan: with no base at all the range says so instead of implying it covered history', async () => {
  const repo = await makeRepo()
  try {
    await git(repo, ['checkout', '-q', '-b', 'lonely'])
    await writeFile(join(repo, 'solo.ts'), 'export const solo = 1\n')
    await git(repo, ['add', '-A'])
    await git(repo, ['commit', '-q', '-m', 'solo'])
    const candidate = await git(repo, ['rev-parse', 'HEAD'])

    const range = await listCommitsToPublish({
      repoRoot: repo,
      candidate,
      remoteName: 'origin',
      remoteBranch: 'main',
      fallbackBase: 'does-not-exist',
    })
    assert.equal(range.source, 'candidate-only')
    assert.equal(range.base, null)
    assert.deepEqual(range.commits, [candidate], 'the candidate is always scanned, base or not')
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})
