// ---------------------------------------------------------------------------
// ENG-21 — Isolated Worker Worktree Execution: provisioner.
//
// Boring local Git mechanisms only: one worker execution = one unique local
// branch + one unique worktree OUTSIDE the primary checkout. The primary
// checkout is never a worker scratch directory; this module never
// stashes/resets/cleans/checks-out-over another worker's files and never
// rewrites commits.
//
// Naming:
//   branch        agent/<story-id>/<run-id>
//   worktree      <worktreesRoot>/<story-id>-<run-id>
//   worktreesRoot default ../Culebraluxe-worktrees next to the primary checkout
//
// NO auto merge / rebase / push / PR behavior exists anywhere in this module.
// Integration is an explicit human-controlled checkpoint (ENG-22).
// ---------------------------------------------------------------------------

import { randomBytes } from 'node:crypto'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readlinkSync,
  rmSync,
  symlinkSync,
} from 'node:fs'
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'

import type {
  WorkerWorkspace,
  WorkerWorkspaceListItem,
  WorkerWorkspaceSpec,
} from './types'
import { WorkerWorkspaceError } from './types'

const execFileAsync = promisify(execFile)

export const GIT_BRANCH_PREFIX = 'agent/'
export const DEFAULT_WORKTREES_DIRNAME = 'Culebraluxe-worktrees'
export const DEFAULT_SHARED_LINKS = ['node_modules', '.env.local'] as const

// ---------------------------------------------------------------------------
// Small git helpers
// ---------------------------------------------------------------------------

async function runGit(cwd: string, args: string[]): Promise<string> {
  try {
    const { stdout } = await execFileAsync('git', args, {
      cwd,
      encoding: 'utf8',
    })
    return stdout.trim()
  } catch (err) {
    const detail = (err as { stderr?: string }).stderr?.trim()
    throw new WorkerWorkspaceError(
      `git ${args.join(' ')} failed: ${detail || (err as Error).message}`,
    )
  }
}

async function gitOk(cwd: string, args: string[]): Promise<boolean> {
  try {
    await execFileAsync('git', args, { cwd })
    return true
  } catch {
    return false
  }
}

/** Resolve the PRIMARY checkout root (the checkout whose `.git` is shared). */
export async function resolveRepoRoot(repoRoot?: string): Promise<string> {
  const cwd = repoRoot ? resolve(repoRoot) : process.cwd()
  await execFileAsync('git', ['rev-parse', '--git-dir'], { cwd })
  const common = (
    await execFileAsync('git', ['rev-parse', '--git-common-dir'], {
      cwd,
      encoding: 'utf8',
    })
  ).stdout.trim()
  const commonDir = isAbsolute(common) ? common : resolve(cwd, common)
  return dirname(commonDir)
}

// ---------------------------------------------------------------------------
// Approved integration base (explicit, fail closed)
// ---------------------------------------------------------------------------

/**
 * Resolve the EXPLICIT approved integration base ref for worker execution.
 *
 * Precedence: the `AGENT_WORKSPACE_BASE_REF` environment override, else the
 * accepted integration tracking ref (`origin/main`). The value is a REF
 * (branch/tag/commit) — never a silent "whatever HEAD is current" — and the
 * provisioner pins it to a fixed commit at creation time. An unresolvable ref
 * fails closed with an actionable error instead of defaulting.
 *
 * Gemini #1 / V5-03R Invariant 8: successor worktrees must branch from the
 * accepted integration state, not a potentially stale local `main` checkout.
 * `publishAcceptedCandidate` synchronizes `refs/remotes/origin/main` after a
 * successful push, so the default base MUST be the tracking ref. Local
 * `refs/heads/main` is intentionally left untouched by publication.
 */
export function resolveApprovedBaseRef(
  env: NodeJS.ProcessEnv = process.env,
): string {
  const explicit = (env.AGENT_WORKSPACE_BASE_REF ?? '').trim()
  return explicit || 'origin/main'
}

// ---------------------------------------------------------------------------
// Naming (deterministic + safe)
// ---------------------------------------------------------------------------

/** Lowercase alphanumeric+hyphen segment, capped, never empty. */
export function sanitizeBranchSegment(input: string, max = 60): string {
  const cleaned = (input ?? '')
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '')
  return (cleaned || 'story').slice(0, max)
}

export function deriveRunId(workerId: string, runId?: string): string {
  void workerId
  const clean = sanitizeBranchSegment(runId ?? '')
  if (clean) return clean
  const stamp = Date.now().toString(36)
  const rand = randomBytes(3).toString('hex')
  return `run-${stamp}-${rand}`
}

export function deriveBranchName(storyId: string, runId: string): string {
  return `${GIT_BRANCH_PREFIX}${sanitizeBranchSegment(storyId)}/${sanitizeBranchSegment(runId, 40)}`
}

export function deriveWorktreePath(
  worktreesRoot: string,
  storyId: string,
  runId: string,
): string {
  return join(
    worktreesRoot,
    `${sanitizeBranchSegment(storyId)}-${sanitizeBranchSegment(runId)}`,
  )
}

// ---------------------------------------------------------------------------
// Provision
// ---------------------------------------------------------------------------

/**
 * Create one isolated worker workspace:
 *   - resolve the EXPLICIT approved base ref to a fixed commit (fail closed)
 *   - guard: branch must not already exist / path must not already exist
 *   - `git worktree add -b <branch> <path> <baseCommit>` (the primary checkout
 *     is only READ — never a worker scratch directory)
 *   - symlink shared-safe resources (node_modules / .env.local) so the worker
 *     does not reinstall deps or re-create local config
 */
export async function provisionWorkerWorkspace(
  spec: WorkerWorkspaceSpec,
): Promise<WorkerWorkspace> {
  const repoRoot = await resolveRepoRoot(spec.repoRoot)

  const baseRef = (spec.baseRef ?? '').trim()
  if (!baseRef) {
    throw new WorkerWorkspaceError(
      'baseRef is required: pass an explicit approved integration base (branch, tag, or commit hash).',
    )
  }
  let baseCommit: string
  try {
    baseCommit = await runGit(repoRoot, [
      'rev-parse',
      '--verify',
      `${baseRef}^{commit}`,
    ])
  } catch (err) {
    throw new WorkerWorkspaceError(
      `base ref ${JSON.stringify(baseRef)} could not be resolved to a commit: ${(err as Error).message}`,
    )
  }
  if (!/^[0-9a-f]{40}$/.test(baseCommit)) {
    throw new WorkerWorkspaceError(
      `base ref ${JSON.stringify(baseRef)} could not be resolved to a commit.`,
    )
  }

  const storyId = sanitizeBranchSegment(spec.storyId)
  const runId = deriveRunId(spec.workerId, spec.runId)
  const branchName = deriveBranchName(storyId, runId)

  const worktreesRoot = resolve(
    spec.worktreesRoot ??
      join(dirname(repoRoot), DEFAULT_WORKTREES_DIRNAME),
  )
  const worktreePath = deriveWorktreePath(worktreesRoot, storyId, runId)

  // Safety guards — fail clearly before touching anything.
  if (
    worktreesRoot === repoRoot ||
    worktreesRoot.startsWith(repoRoot + sep)
  ) {
    throw new WorkerWorkspaceError(
      `worktreesRoot must be OUTSIDE the primary checkout: ${worktreesRoot}`,
    )
  }
  if (worktreePath.startsWith(repoRoot + sep)) {
    throw new WorkerWorkspaceError(
      `worktree path must be outside the primary checkout: ${worktreePath}`,
    )
  }
  if (
    await gitOk(repoRoot, [
      'show-ref',
      '--verify',
      '--quiet',
      `refs/heads/${branchName}`,
    ])
  ) {
    throw new WorkerWorkspaceError(
      `branch already exists unexpectedly: ${branchName}`,
    )
  }
  if (existsSync(worktreePath)) {
    throw new WorkerWorkspaceError(
      `worktree path already exists unexpectedly: ${worktreePath}`,
    )
  }

  // Create the branch + worktree from the pinned base commit.
  mkdirSync(worktreesRoot, { recursive: true })
  await runGit(repoRoot, [
    'worktree',
    'add',
    '-b',
    branchName,
    worktreePath,
    baseCommit,
  ])

  // Symlink shared-safe resources (relative symlinks — never copies).
  const sharedLinks: string[] = []
  for (const name of spec.sharedLinks ?? [...DEFAULT_SHARED_LINKS]) {
    const target = join(repoRoot, name)
    const link = join(worktreePath, name)
    if (!existsSync(target)) continue
    let already: boolean
    try {
      lstatSync(link)
      already = true
    } catch {
      already = false
    }
    if (already) continue
    const isDir = lstatSync(target).isDirectory()
    symlinkSync(relative(worktreePath, target), link, isDir ? 'dir' : 'file')
    sharedLinks.push(name)
  }

  return {
    storyId,
    workerId: spec.workerId,
    runId,
    branchName,
    worktreePath,
    baseRef,
    baseCommit,
    sharedLinks,
  }
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

/** List every worker workspace (branches under `agent/`) across all worktrees. */
export async function listWorkerWorkspaces(
  opts?: { repoRoot?: string },
): Promise<WorkerWorkspaceListItem[]> {
  const repoRoot = await resolveRepoRoot(opts?.repoRoot)
  const out = await runGit(repoRoot, ['worktree', 'list', '--porcelain'])
  const items: WorkerWorkspaceListItem[] = []
  let cur: { path?: string; branch?: string; head?: string } = {}

  const flush = (): void => {
    if (cur.path && cur.branch?.startsWith(GIT_BRANCH_PREFIX)) {
      const parts = cur.branch.split('/')
      items.push({
        branchName: cur.branch,
        worktreePath: cur.path,
        head: cur.head ?? '',
        storyId: parts[1] ?? null,
        runId: parts.slice(2).join('/') || null,
      })
    }
    cur = {}
  }

  for (const line of out.split('\n')) {
    if (line.startsWith('worktree ')) {
      cur.path = line.slice('worktree '.length)
    } else if (line.startsWith('branch ')) {
      cur.branch = line
        .slice('branch '.length)
        .replace(/^refs\/heads\//, '')
    } else if (line.startsWith('HEAD ')) {
      cur.head = line.slice('HEAD '.length)
    } else if (line === '') {
      flush()
    }
  }
  flush()
  return items
}

// ---------------------------------------------------------------------------
// Commit evidence (honest: never fabricate a worker commit)
// ---------------------------------------------------------------------------

/**
 * Read the worker's checkout HEAD as commit evidence (ENG-21).
 *
 * Honest-by-contract: when the workspace is still exactly at the pinned base
 * commit, the worker created NO commit, so this returns null (a base commit is
 * never persisted as if the worker had committed). When the worker committed
 * (HEAD advanced past the base), the HEAD hash is returned. A git read failure
 * returns null (evidence is factual, never guessed).
 */
export async function readWorkerCommitHash(
  worktreePath: string,
  baseCommit?: string | null,
): Promise<string | null> {
  let head: string
  try {
    head = await runGit(worktreePath, ['rev-parse', 'HEAD'])
  } catch {
    return null
  }
  if (baseCommit && head === baseCommit) return null
  return head
}

// ---------------------------------------------------------------------------
// Cleanup (conservative)
// ---------------------------------------------------------------------------

export type RemoveWorkerWorkspaceOptions = {
  storyId: string
  runId?: string
  repoRoot?: string
}

/**
 * Safe cleanup. Removes a worktree ONLY when the worker has no uncommitted
 * tracked changes and no unknown (non-ignored) files would be destroyed.
 * NEVER force-deletes: an abandoned workspace is cheaper than lost code. The
 * branch (and all its commits) is ALWAYS preserved.
 *
 * Order of operations (a refused removal is a no-op):
 *   1. dirty check FIRST — tracked modifications or untracked non-ignored
 *      files refuse before anything is touched
 *   2. remove ONLY the shared symlinks this provisioner created (verified by
 *      resolved target); the primary checkout's targets are never touched
 *   3. `git worktree remove` — no --force
 *   4. `git worktree prune`
 */
export async function removeWorkerWorkspace(
  opts: RemoveWorkerWorkspaceOptions,
): Promise<{ removedPath: string; preservedBranch: string }> {
  const repoRoot = await resolveRepoRoot(opts.repoRoot)
  const storyId = sanitizeBranchSegment(opts.storyId)
  const items = await listWorkerWorkspaces({ repoRoot })
  const matches = items.filter(
    (i) =>
      i.storyId === storyId &&
      (opts.runId ? i.runId === opts.runId : true),
  )
  if (matches.length === 0) {
    throw new WorkerWorkspaceError(
      `no agent workspace found for story ${opts.storyId}${opts.runId ? ` / run ${opts.runId}` : ''}`,
    )
  }
  const target = opts.runId
    ? matches[0]
    : matches.length === 1
      ? matches[0]
      : (() => {
          throw new WorkerWorkspaceError(
            `multiple agent workspaces exist for story ${opts.storyId}; pass --run <id> to disambiguate: ${matches.map((m) => m.runId).join(', ')}`,
          )
        })()

  const worktreePath = target.worktreePath
  if (!existsSync(worktreePath)) {
    throw new WorkerWorkspaceError(`worktree path missing: ${worktreePath}`)
  }

  // 1. Refuse while any tracked/untracked non-ignored change remains.
  const dirty = await runGit(worktreePath, ['status', '--porcelain'])
  if (dirty) {
    throw new WorkerWorkspaceError(
      `workspace ${worktreePath} has uncommitted changes; refusing to remove (an abandoned workspace is cheaper than lost code). Commit the changes on branch ${target.branchName} first.`,
    )
  }

  // 2. Remove ONLY the shared symlinks the provisioner created (verified by
  //    resolved target). Unknown/untracked files are NEVER touched here.
  for (const name of DEFAULT_SHARED_LINKS) {
    const link = join(worktreePath, name)
    try {
      const read = readlinkSync(link)
      if (resolve(worktreePath, read) === resolve(repoRoot, name)) {
        rmSync(link)
      }
    } catch {
      // Not our symlink or already gone — leave it alone.
    }
  }

  // 3. `git worktree remove` — no --force. The branch survives.
  try {
    await runGit(repoRoot, ['worktree', 'remove', worktreePath])
  } catch (err) {
    throw new WorkerWorkspaceError(
      `git refused to remove ${worktreePath}: ${(err as Error).message}. Committed work is preserved on branch ${target.branchName}; remove the remaining local files (e.g. generated .next/, node_modules/) manually before retrying — never force-delete.`,
    )
  }
  await runGit(repoRoot, ['worktree', 'prune'])

  return { removedPath: worktreePath, preservedBranch: target.branchName }
}
