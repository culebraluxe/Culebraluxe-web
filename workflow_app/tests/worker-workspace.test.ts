// ---------------------------------------------------------------------------
// ENG-21 — Isolated Worker Worktree Execution: SCOPED focused proofs.
//
// Controlled temp-repo integration tests (never touch active repository state;
// zero Neon):
//   1. UNIQUE provisioning — one worker/run = one unique branch + worktree,
//      deterministic naming, HEAD pinned to the base commit
//   2. CONCURRENT ISOLATION — two workspaces coexist; changes in A never
//      appear as dirty state in B or the primary checkout; independent commits
//      and NO auto-merge/push (the primary branch tip never advances)
//   3. EXPLICIT BASE REF — workspace pins to the supplied ref; unresolvable
//      ref fails clearly (never HEAD-derived)
//   4. DIRTY PRIMARY — a dirty primary checkout does not block isolated
//      workspace creation and is never touched (no stash/reset/clean)
//   5. HONEST COMMIT EVIDENCE — readWorkerCommitHash returns null when the
//      checkout is still at the base (no worker commit), the HEAD hash after a
//      worker commit, null on unreadable paths (evidence is never fabricated)
//   6. SAFE CLEANUP — refuses destructive removal of uncommitted work; removes
//      ONLY the provisioner's own shared symlinks; the branch (and all
//      commits) is always preserved
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdir, mkdtemp, realpath, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { promisify } from 'node:util'

import {
  listWorkerWorkspaces,
  provisionWorkerWorkspace,
  readWorkerCommitHash,
  removeWorkerWorkspace,
  resolveRepoRoot,
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
  // Mirror the real repo's ignores so shared symlinks (node_modules /
  // .env.local / .next) never make an isolated workspace look dirty.
  await writeFile(
    join(repoRoot, '.gitignore'),
    'node_modules\n.env.local\n.next\n',
  )
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

test('ENG-21: provision creates a unique deterministic branch + worktree per worker/run', async () => {
  const t = await makeTempRepo()
  try {
    const baseCommit = await git(t.repoRoot, ['rev-parse', 'HEAD'])
    const a = await provisionWorkerWorkspace(spec(t, 'cmd-02', 'run-a'))
    const b = await provisionWorkerWorkspace(spec(t, 'cmd-02', 'run-b'))

    // Deterministic naming: agent/<story>/<run>.
    assert.equal(a.branchName, 'agent/cmd-02/run-a')
    assert.equal(b.branchName, 'agent/cmd-02/run-b')
    assert.notEqual(a.branchName, b.branchName)
    assert.notEqual(a.worktreePath, b.worktreePath)
    // The workspace records branch / worktree / base ref / base commit.
    assert.equal(a.baseCommit, baseCommit)
    assert.equal(a.baseRef, 'main')
    assert.equal(a.storyId, 'cmd-02')
    assert.equal(a.workerId, 'eng21-worker')
    assert.equal(a.runId, 'run-a')
    assert.equal(await git(a.worktreePath, ['rev-parse', 'HEAD']), baseCommit)
    assert.equal(await git(b.worktreePath, ['rev-parse', 'HEAD']), baseCommit)
    assert.equal(await git(a.worktreePath, ['branch', '--show-current']), 'agent/cmd-02/run-a')

    const items = await listWorkerWorkspaces({ repoRoot: t.repoRoot })
    assert.equal(items.length, 2)
    assert.ok(items.some((i) => i.branchName === 'agent/cmd-02/run-a'))
    assert.ok(items.some((i) => i.branchName === 'agent/cmd-02/run-b'))
    assert.ok(items.every((i) => i.storyId === 'cmd-02'))
    assert.ok(items.every((i) => i.head === baseCommit))

    // Duplicate provision of the same lane fails clearly (never overwrites).
    await assert.rejects(
      provisionWorkerWorkspace(spec(t, 'cmd-02', 'run-a')),
      (e: unknown) => (e as Error).message.includes('already exists'),
    )
  } finally {
    await destroyTempRepo(t)
  }
})

// ---------------------------------------------------------------------------
// 2. CONCURRENT ISOLATION + no auto-merge/push
// ---------------------------------------------------------------------------

test('ENG-21: two worker workspaces coexist; changes never cross-contaminate; nothing is merged or pushed', async () => {
  const t = await makeTempRepo()
  try {
    const a = await provisionWorkerWorkspace(spec(t, 'eng21a', 'a'))
    const b = await provisionWorkerWorkspace(spec(t, 'eng21b', 'b'))

    // Worker A writes an untracked file.
    await writeFile(join(a.worktreePath, 'worker-a-note.txt'), 'a\n')

    // B and the primary checkout remain clean — A's work is invisible to them.
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

    // NO auto-merge / push: the primary branch tip never advances and no
    // remote-tracking ref appears, even after both workers committed.
    assert.equal(await git(t.repoRoot, ['log', '-1', '--format=%s']), 'base')
    assert.equal(await git(t.repoRoot, ['rev-parse', 'main']), await git(t.repoRoot, ['rev-parse', 'HEAD']))
    assert.equal(await git(t.repoRoot, ['branch', '-r']), '')
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

    // An empty base ref is refused (never silently defaulted by the mechanism).
    await assert.rejects(
      provisionWorkerWorkspace(spec(t, 'eng21c', 'c3', { baseRef: '' })),
      (e: unknown) => (e as Error).message.includes('baseRef is required'),
    )
  } finally {
    await destroyTempRepo(t)
  }
})

// ---------------------------------------------------------------------------
// 4. DIRTY PRIMARY DOES NOT BLOCK
// ---------------------------------------------------------------------------

test('ENG-21: a dirty primary checkout does not block isolated workspace creation and is never touched', async () => {
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
    assert.equal(
      (await git(t.repoRoot, ['show', 'HEAD:README.md'])).trim(),
      'eng21 fixture',
      'primary HEAD unchanged',
    )
  } finally {
    await destroyTempRepo(t)
  }
})

// ---------------------------------------------------------------------------
// 5. HONEST COMMIT EVIDENCE
// ---------------------------------------------------------------------------

test('ENG-21: readWorkerCommitHash is honest — null at base, HEAD after a worker commit, null when unreadable', async () => {
  const t = await makeTempRepo()
  try {
    const ws = await provisionWorkerWorkspace(spec(t, 'eng21f', 'f'))
    const baseCommit = ws.baseCommit

    // No worker commit yet: still at the approved base -> null (never a
    // fabricated hash).
    assert.equal(await readWorkerCommitHash(ws.worktreePath, baseCommit), null)

    // Worker commits: HEAD hash is returned.
    await writeFile(join(ws.worktreePath, 'evidence.txt'), 'done\n')
    await git(ws.worktreePath, ['add', '.'])
    await git(ws.worktreePath, ['commit', '-m', 'worker commit', '-q'])
    const head = await git(ws.worktreePath, ['rev-parse', 'HEAD'])
    assert.notEqual(head, baseCommit)
    assert.equal(await readWorkerCommitHash(ws.worktreePath, baseCommit), head)

    // Unreadable path -> null (evidence is factual, never guessed).
    assert.equal(await readWorkerCommitHash('/no/such/path', baseCommit), null)
  } finally {
    await destroyTempRepo(t)
  }
})

// ---------------------------------------------------------------------------
// 6. SAFE CLEANUP
// ---------------------------------------------------------------------------

test('ENG-21: cleanup refuses destructive removal, removes only its own symlinks, and always preserves the branch', async () => {
  const t = await makeTempRepo()
  try {
    // Give the primary checkout shared resources so the provisioner symlinks
    // them into the workspace.
    await mkdir(join(t.repoRoot, 'node_modules'), { recursive: true })
    await writeFile(join(t.repoRoot, 'node_modules', 'dep.txt'), 'dep\n')
    await writeFile(join(t.repoRoot, '.env.local'), 'LOCAL=1\n')

    const ws = await provisionWorkerWorkspace(spec(t, 'eng21e', 'e'))
    assert.ok(ws.sharedLinks.includes('node_modules'))
    assert.ok(ws.sharedLinks.includes('.env.local'))

    // Uncommitted work -> refuse; the workspace (and its symlinks) survive.
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

    // Clean the untracked file, then commit real work.
    await git(ws.worktreePath, ['clean', '-f', '-q'])
    await writeFile(join(ws.worktreePath, 'finished.txt'), 'done\n')
    await git(ws.worktreePath, ['add', '.'])
    await git(ws.worktreePath, ['commit', '-m', 'finish e', '-q'])

    // Removal succeeds; ONLY the provisioner's symlinks are removed; the
    // primary checkout's targets are untouched.
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
    // Primary shared resources intact.
    assert.equal(
      (await git(t.repoRoot, ['status', '--porcelain'])).includes('node_modules'),
      false,
    )
    const dep = await execFileAsync('cat', [join(t.repoRoot, 'node_modules', 'dep.txt')], { encoding: 'utf8' })
    assert.equal(dep.stdout.trim(), 'dep')
  } finally {
    await destroyTempRepo(t)
  }
})

// ---------------------------------------------------------------------------
// 7. Structure guards
// ---------------------------------------------------------------------------

test('ENG-21: worktrees root must be outside the primary checkout', async () => {
  const t = await makeTempRepo()
  try {
    await assert.rejects(
      provisionWorkerWorkspace(
        spec(t, 'eng21g', 'g', { worktreesRoot: join(t.repoRoot, 'inside') }),
      ),
      (e: unknown) => (e as Error).message.includes('OUTSIDE the primary checkout'),
    )
  } finally {
    await destroyTempRepo(t)
  }
})

test('ENG-21: resolveRepoRoot resolves the primary checkout from a worktree path', async () => {
  const t = await makeTempRepo()
  try {
    const ws = await provisionWorkerWorkspace(spec(t, 'eng21h', 'h'))
    // From INSIDE the worktree, the primary checkout root is still resolved.
    // (realpath: macOS /tmp and /var are symlinks — compare canonical paths.)
    assert.equal(await resolveRepoRoot(ws.worktreePath), await realpath(t.repoRoot))
  } finally {
    await destroyTempRepo(t)
  }
})
