import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { promisify } from 'node:util'

import { sweepLandedLaneBranches } from '@/lib/worker-workspace/lane-teardown'

const execFileAsync = promisify(execFile)

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

/** A repo on `main` with one commit, plus a lane-style branch name ready to be filled in. */
async function makeRepo(): Promise<string> {
  const repo = await mkdtemp(join(tmpdir(), 'forge-sweep-'))
  await git(repo, ['init', '-b', 'main', '-q'])
  await git(repo, ['config', 'user.name', 'fence'])
  await git(repo, ['config', 'user.email', 'fence@test'])
  await git(repo, ['config', 'commit.gpgsign', 'false'])
  await writeFile(join(repo, 'app.ts'), 'export const app = 1\n')
  await git(repo, ['add', '-A'])
  await git(repo, ['commit', '-q', '-m', 'base'])
  return repo
}

async function branches(repo: string): Promise<string[]> {
  return (await git(repo, ['branch', '--format=%(refname:short)'])).split('\n').filter(Boolean)
}

/**
 * A lane branch whose change ALSO reached main — the shape a published candidate leaves behind. Built
 * with an explicit cherry-pick so main holds the same patch under a different sha, which is exactly the
 * case `git branch --merged` could never see and the whole reason this sweep judges by patch.
 */
async function landLaneWork(repo: string, branch: string, file: string, body: string): Promise<void> {
  await git(repo, ['checkout', '-q', '-b', branch, 'main'])
  await writeFile(join(repo, file), body)
  await git(repo, ['add', '-A'])
  await git(repo, ['commit', '-q', '-m', `lane work in ${file}`])
  const tip = await git(repo, ['rev-parse', 'HEAD'])
  await git(repo, ['checkout', '-q', 'main'])
  await git(repo, ['cherry-pick', '--no-commit', tip])
  await git(repo, ['commit', '-q', '-m', `land ${file}`])
}

// ---------------------------------------------------------------------------
// ENG-FORGE-LANE-TEARDOWN-01 — the machine cleans up after itself when its work lands.
//
// 168 branches had accumulated on this repository by 2026-09-18: a branch per lane attempt, plus one
// per dogfood and smoke rehearsal. `removeWorkerWorkspace` preserves the branch on purpose (an abandoned
// lane's commits are cheaper than lost code) and nothing ever collected them. This sweep runs at the one
// moment the answer is certain — a candidate has just been published — and still re-derives the proof
// instead of assuming it, because a refused candidate's branch must survive to be looked at.
// ---------------------------------------------------------------------------

test('lane-teardown: a landed lane branch is deleted after its work is on main', async () => {
  const repo = await makeRepo()
  try {
    await landLaneWork(repo, 'agent/doc-08/e1', 'app.ts', 'export const app = 2\n')
    const sweep = await sweepLandedLaneBranches({ repoRoot: repo })
    assert.deepEqual(sweep.deleted, ['agent/doc-08/e1'])
    assert.equal((await branches(repo)).includes('agent/doc-08/e1'), false)
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('lane-teardown: a lane whose work did NOT land is kept — a refused candidate is looked at, not lost', async () => {
  const repo = await makeRepo()
  try {
    await git(repo, ['checkout', '-q', '-b', 'agent/eng-forge-v5-11/c67cdae5', 'main'])
    await writeFile(join(repo, 'unlanded.ts'), 'export const x = 1\n')
    await git(repo, ['add', '-A'])
    await git(repo, ['commit', '-q', '-m', 'work that never landed'])
    await git(repo, ['checkout', '-q', 'main'])

    const sweep = await sweepLandedLaneBranches({ repoRoot: repo })
    assert.deepEqual(sweep.deleted, [])
    assert.equal(sweep.kept, 1)
    assert.equal((await branches(repo)).includes('agent/eng-forge-v5-11/c67cdae5'), true)
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('lane-teardown: a branch a PERSON named is never touched, landed or not', async () => {
  const repo = await makeRepo()
  try {
    await landLaneWork(repo, 'feat/marketing-next20', 'app.ts', 'export const app = 3\n')
    const sweep = await sweepLandedLaneBranches({ repoRoot: repo })
    assert.deepEqual(sweep.deleted, [])
    assert.equal((await branches(repo)).includes('feat/marketing-next20'), true)
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('lane-teardown: the base branch itself is never a candidate', async () => {
  const repo = await makeRepo()
  try {
    const sweep = await sweepLandedLaneBranches({ repoRoot: repo })
    assert.deepEqual(sweep.deleted, [])
    assert.equal((await branches(repo)).includes('main'), true)
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})
