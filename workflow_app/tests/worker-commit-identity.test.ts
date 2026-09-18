import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { promisify } from 'node:util'

import { FORGE_COMMIT_IDENTITY, commitWorkerWorkspaceChanges } from '../../lib/worker-workspace/commit'

const execFileAsync = promisify(execFile)

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

/**
 * A repo whose CONFIG IS THE BUG. This is the environment that produced 228 commits authored by
 * `Your Name <you@example.com>` on the real repository (2026-08-23..28): no real identity, so git
 * falls back to its placeholder. The old fence configured `user.email = eng21@test` and therefore
 * proved a setup the engine never had, which is why the defect survived a green suite.
 */
async function makeRepoWithPlaceholderIdentity(): Promise<string> {
  const repo = await mkdtemp(join(tmpdir(), 'forge-identity-'))
  await git(repo, ['init', '-b', 'main', '-q'])
  await git(repo, ['config', 'user.name', 'Your Name'])
  await git(repo, ['config', 'user.email', 'you@example.com'])
  await git(repo, ['config', 'commit.gpgsign', 'false'])
  return repo
}

test('commit identity: the worker commit is stamped with the forge identity, not the machine placeholder', async () => {
  const repo = await makeRepoWithPlaceholderIdentity()
  try {
    await writeFile(join(repo, 'work.txt'), 'the lane changed this\n')
    const result = await commitWorkerWorkspaceChanges(repo, 'lane: work')

    assert.equal(result.changed, true)
    assert.ok(result.commitHash, 'a commit was made')

    const author = await git(repo, ['log', '-1', '--format=%an|%ae|%cn|%ce'])
    const [authorName, authorEmail, committerName, committerEmail] = author.split('|')
    assert.equal(authorName, FORGE_COMMIT_IDENTITY.name)
    assert.equal(authorEmail, FORGE_COMMIT_IDENTITY.email)
    assert.equal(committerName, FORGE_COMMIT_IDENTITY.name)
    assert.equal(committerEmail, FORGE_COMMIT_IDENTITY.email)
    assert.notEqual(authorEmail, 'you@example.com', 'the placeholder machine identity must never author a commit')
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('commit identity: the scoped path is stamped the same way', async () => {
  const repo = await makeRepoWithPlaceholderIdentity()
  try {
    await writeFile(join(repo, 'in-scope.txt'), 'declared\n')
    const result = await commitWorkerWorkspaceChanges(repo, 'lane: scoped work', {
      allowedScope: ['in-scope.txt'],
    })
    assert.equal(result.changed, true)
    const author = await git(repo, ['log', '-1', '--format=%ae'])
    assert.equal(author, FORGE_COMMIT_IDENTITY.email)
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('commit identity: nothing to commit is still reported as nothing, and writes no commit', async () => {
  const repo = await makeRepoWithPlaceholderIdentity()
  try {
    const result = await commitWorkerWorkspaceChanges(repo, 'lane: no changes')
    assert.equal(result.changed, false)
    assert.equal(result.commitHash, null)
    await assert.rejects(git(repo, ['log', '-1']), 'no commit exists yet')
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})
