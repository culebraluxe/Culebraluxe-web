// ---------------------------------------------------------------------------
// ENG-21 — Isolated Worker Worktree Execution: SCOPED focused proofs (5).
//
// Controlled temp-repo integration tests (never touch active repository state):
//   1. UNIQUE provisioning — one worker/run = one unique branch + worktree
//   2. CONCURRENT ISOLATION — two workspaces coexist; changes in A never
//      appear as dirty state in B or the primary checkout; independent commits
//   3. EXPLICIT BASE REF — workspace pins to the supplied ref; unresolvable
//      ref fails clearly (never HEAD-derived)
//   4. DIRTY PRIMARY — a dirty primary checkout does not block isolated
//      workspace creation (no stash/reset/clean)
//   5. SAFE CLEANUP — refuses destructive removal of uncommitted work; the
//      branch (and commits) is always preserved
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { promisify } from 'node:util'

import {
  listWorkerWorkspaces,
  provisionWorkerWorkspace,
  removeWorkerWorkspace,
} from '../../lib/worker-workspace'

const execFileAsync = promisify(execFile)

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

type TempRepo = { repoRoot: string; worktreesRoot: string }

async function makeTempRepo(): Promise<TempRepo> {
  const parent = await mkdtemp(join(tmpdir(), 'eng21-'))
  const repoRoot = join(parent, 'repo')
  const worktreesRoot = join(parent, 'worktrees')
  await mkdir(repoRoot, { recursive: true })
  await mkdir(worktreesRoot, { recursive: true })
  await git(repoRoot, ['init', '-b', 'main', '-q'])
  await git(repoRoot, ['config', 'user.email', 'eng21@test'])
  await git(repoRoot, ['config', 'user.name', 'eng21'])
  await writeFile(join(repoRoot, 'README.md'), 'eng21 fixture\n')
  await git(repoRoot, ['add', '.'])
  await git(repoRoot, ['commit', '-m', 'base', '-q'])
  return { repoRoot, worktreesRoot }
}

async function destroyTempRepo(t: TempRepo): Promise<void> {
  await rm(dirname(t.repoRoot), { recursive: true, force: true })
}

const spec = (
  t: TempRepo,
  storyId: string,
  runId: string,
  overrides: Partial<Parameters<typeof provisionWorkerWorkspace>[0]> = {},
) => ({
  storyId,
  workerId: 'eng21-worker',
  baseRef: 'main',
  runId,
  repoRoot: t.repoRoot,
  worktreesRoot: t.worktreesRoot,
  ...overrides,
})

// ---------------------------------------------------------------------------
// 1. UNIQUE provisioning
// ---------------------------------------------------------------------------

test('ENG-21: provision creates a unique branch + worktree per worker/run', async () => {
  const t = await makeTempRepo()
  try {
    const baseCommit = await git(t.repoRoot, ['rev-parse', 'HEAD'])
    const a = await provisionWorkerWorkspace(spec(t, 'cmd-02', 'run-a'))
    const b = await provisionWorkerWorkspace(spec(t, 'cmd-02', 'run-b'))

    assert.equal(a.branchName, 'agent/cmd-02/run-a')
    assert.equal(b.branchName, 'agent/cmd-02/run-b')
    assert.notEqual(a.branchName, b.branchName)
    assert.notEqual(a.worktreePath, b.worktreePath)
    assert.equal(a.baseCommit, baseCommit)
    assert.equal(await git(a.worktreePath, ['rev-parse', 'HEAD']), baseCommit)
    assert.equal(await git(b.worktreePath, ['rev-parse', 'HEAD']), baseCommit)

    const items = await listWorkerWorkspaces({ repoRoot: t.repoRoot })
    assert.equal(items.length, 2)
    assert.ok(items.some((i) => i.branchName === 'agent/cmd-02/run-a'))
    assert.ok(items.some((i) => i.branchName === 'agent/cmd-02/run-b'))
  } finally {
    await destroyTempRepo(t)
  }
})

// ---------------------------------------------------------------------------
// 2. CONCURRENT ISOLATION
// ---------------------------------------------------------------------------

test('ENG-21: two worker workspaces coexist; changes never cross-contaminate', async () => {
  const t = await makeTempRepo()
  try {
    const a = await provisionWorkerWorkspace(spec(t, 'eng21a', 'a'))
    const b = await provisionWorkerWorkspace(spec(t, 'eng21b', 'b'))

    // Worker A writes an untracked file.
    await writeFile(join(a.worktreePath, 'worker-a-note.txt'), 'a\n')

    // B and the primary checkout remain clean.
    assert.equal(await git(b.worktreePath, ['status', '--porcelain']), '')
    assert.equal(await git(t.repoRoot, ['status', '--porcelain']), '')

    // Each worker commits independently on its own branch.
    await git(a.worktreePath, ['add', '.'])
    await git(a.worktreePath, ['commit', '-m', 'a work', '-q'])
    await writeFile(join(b.worktreePath, 'worker-b-note.txt'), 'b\n')
    await git(b.worktreePath, ['add', '.'])
    await git(b.worktreePath, ['commit', '-m', 'b work', '-q'])

    assert.equal(await git(a.worktreePath, ['log', '-1', '--format=%s']), 'a work')
    assert.equal(await git(b.worktreePath, ['log', '-1', '--format=%s']), 'b work')
    assert.notEqual(
      await git(a.worktreePath, ['rev-parse', 'HEAD']),
      await git(b.worktreePath, ['rev-parse', 'HEAD']),
    )
    // Advancing one worker never mutates the other or the primary checkout.
    assert.equal(await git(t.repoRoot, ['log', '-1', '--format=%s']), 'base')
  } finally {
    await destroyTempRepo(t)
  }
})

// ---------------------------------------------------------------------------
// 3. EXPLICIT BASE REF
// ---------------------------------------------------------------------------

test('ENG-21: explicit base ref pins the workspace; unresolvable ref fails clearly', async () => {
  const t = await makeTempRepo()
  try {
    await git(t.repoRoot, ['checkout', '-b', 'base-branch', '-q'])
    await writeFile(join(t.repoRoot, 'base.txt'), 'base work\n')
    await git(t.repoRoot, ['add', '.'])
    await git(t.repoRoot, ['commit', '-m', 'base branch work', '-q'])
    const baseCommit = await git(t.repoRoot, ['rev-parse', 'HEAD'])
    await git(t.repoRoot, ['checkout', 'main', '-q'])

    const ws = await provisionWorkerWorkspace(
      spec(t, 'eng21c', 'c', { baseRef: 'base-branch' }),
    )
    assert.equal(ws.baseCommit, baseCommit)
    assert.equal(await git(ws.worktreePath, ['rev-parse', 'HEAD']), baseCommit)
    assert.match(await git(ws.worktreePath, ['show', 'HEAD:base.txt']), /base work/)

    await assert.rejects(
      provisionWorkerWorkspace(
        spec(t, 'eng21c', 'c2', { baseRef: 'no-such-ref-xyz' }),
      ),
      (e: unknown) => (e as Error).message.includes('could not be resolved'),
    )
  } finally {
    await destroyTempRepo(t)
  }
})

// ---------------------------------------------------------------------------
// 4. DIRTY PRIMARY DOES NOT BLOCK
// ---------------------------------------------------------------------------

test('ENG-21: a dirty primary checkout does not block isolated workspace creation', async () => {
  const t = await makeTempRepo()
  try {
    await writeFile(join(t.repoRoot, 'untracked.txt'), 'dirty\n')
    await writeFile(join(t.repoRoot, 'README.md'), 'modified\n')

    const ws = await provisionWorkerWorkspace(spec(t, 'eng21d', 'd'))

    // The isolated workspace is clean and has the BASE content.
    assert.equal(await git(ws.worktreePath, ['status', '--porcelain']), '')
    assert.equal(
      (await git(ws.worktreePath, ['show', 'HEAD:README.md'])).trim(),
      'eng21 fixture',
    )

    // The primary checkout is untouched (still dirty — never stashed/reset).
    const primaryDirty = await git(t.repoRoot, ['status', '--porcelain'])
    assert.ok(primaryDirty.includes('untracked.txt'))
    assert.ok(primaryDirty.includes('README.md'))
  } finally {
    await destroyTempRepo(t)
  }
})

// ---------------------------------------------------------------------------
// 5. SAFE CLEANUP
// ---------------------------------------------------------------------------

test('ENG-21: cleanup refuses destructive removal and always preserves the branch', async () => {
  const t = await makeTempRepo()
  try {
    const ws = await provisionWorkerWorkspace(spec(t, 'eng21e', 'e'))

    // Uncommitted work -> refuse; the workspace survives.
    await writeFile(join(ws.worktreePath, 'work-in-progress.txt'), 'wip\n')
    await assert.rejects(
      removeWorkerWorkspace({
        storyId: 'eng21e',
        runId: 'e',
        repoRoot: t.repoRoot,
      }),
      (e: unknown) => (e as Error).message.includes('uncommitted changes'),
    )
    assert.equal((await listWorkerWorkspaces({ repoRoot: t.repoRoot })).length, 1)

    // Committed work -> removal succeeds; branch + commit preserved.
    await git(ws.worktreePath, ['add', '.'])
    await git(ws.worktreePath, ['commit', '-m', 'finish e', '-q'])
    const result = await removeWorkerWorkspace({
      storyId: 'eng21e',
      runId: 'e',
      repoRoot: t.repoRoot,
    })
    assert.equal(result.preservedBranch, 'agent/eng21e/e')
    assert.equal((await listWorkerWorkspaces({ repoRoot: t.repoRoot })).length, 0)

    const branchCommit = await git(t.repoRoot, ['rev-parse', 'agent/eng21e/e'])
    assert.equal(
      await git(t.repoRoot, ['log', '-1', '--format=%s', branchCommit]),
      'finish e',
    )
  } finally {
    await destroyTempRepo(t)
  }
})

