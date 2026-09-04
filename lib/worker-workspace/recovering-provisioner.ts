import { execFile } from 'node:child_process'
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readlinkSync,
  symlinkSync,
} from 'node:fs'
import { dirname, join, relative, resolve, sep } from 'node:path'
import { promisify } from 'node:util'

import {
  DEFAULT_SHARED_LINKS,
  DEFAULT_WORKTREES_DIRNAME,
  deriveBranchName,
  deriveRunId,
  deriveWorktreePath,
  listWorkerWorkspaces,
  provisionWorkerWorkspace,
  resolveRepoRoot,
  sanitizeBranchSegment,
} from './provisioner'
import type { WorkerWorkspace, WorkerWorkspaceSpec } from './types'
import { WorkerWorkspaceError } from './types'

const execFileAsync = promisify(execFile)

async function runGit(cwd: string, args: string[]): Promise<string> {
  try {
    const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
    return stdout.trim()
  } catch (err) {
    const detail = (err as { stderr?: string }).stderr?.trim()
    throw new WorkerWorkspaceError(
      `git ${args.join(' ')} failed: ${detail || (err as Error).message}`,
    )
  }
}

async function branchExists(repoRoot: string, branchName: string): Promise<boolean> {
  try {
    await execFileAsync(
      'git',
      ['show-ref', '--verify', '--quiet', `refs/heads/${branchName}`],
      { cwd: repoRoot },
    )
    return true
  } catch {
    return false
  }
}

function assertExternalWorktreeRoot(repoRoot: string, worktreesRoot: string, worktreePath: string): void {
  if (worktreesRoot === repoRoot || worktreesRoot.startsWith(repoRoot + sep)) {
    throw new WorkerWorkspaceError(
      `worktreesRoot must be OUTSIDE the primary checkout: ${worktreesRoot}`,
    )
  }
  if (worktreePath.startsWith(repoRoot + sep)) {
    throw new WorkerWorkspaceError(
      `worktree path must be outside the primary checkout: ${worktreePath}`,
    )
  }
}

async function ensureSharedLinks(
  repoRoot: string,
  worktreePath: string,
  names: readonly string[],
): Promise<string[]> {
  const sharedLinks: string[] = []
  for (const name of names) {
    const target = join(repoRoot, name)
    const link = join(worktreePath, name)
    if (!existsSync(target)) continue

    try {
      const stat = lstatSync(link)
      if (!stat.isSymbolicLink()) {
        throw new WorkerWorkspaceError(
          `recovered workspace has a non-symlink at shared resource ${link}; refusing to replace it.`,
        )
      }
      const existingTarget = resolve(worktreePath, readlinkSync(link))
      if (existingTarget !== resolve(target)) {
        throw new WorkerWorkspaceError(
          `recovered workspace shared link ${link} points to ${existingTarget}, expected ${resolve(target)}.`,
        )
      }
      sharedLinks.push(name)
      continue
    } catch (err) {
      if (err instanceof WorkerWorkspaceError) throw err
      // Missing link: recreate only the known shared-safe resource.
    }

    const isDir = lstatSync(target).isDirectory()
    symlinkSync(relative(worktreePath, target), link, isDir ? 'dir' : 'file')
    sharedLinks.push(name)
  }
  return sharedLinks
}

/**
 * Attach to the deterministic Forge workspace for this work item when it
 * already exists; otherwise create it normally.
 *
 * This is the recovery counterpart to provisionWorkerWorkspace. A crashed
 * OpenCode process may leave dirty files or a local commit in the worktree.
 * Those files ARE the work product and must survive the next process launch.
 * Recovery therefore accepts only the exact expected branch/path pair and
 * never cleans, resets, rebases, or recreates it.
 */
export async function provisionOrRecoverWorkerWorkspace(
  spec: WorkerWorkspaceSpec,
): Promise<WorkerWorkspace> {
  const repoRoot = await resolveRepoRoot(spec.repoRoot)
  const baseRef = (spec.baseRef ?? '').trim()
  if (!baseRef) {
    throw new WorkerWorkspaceError(
      'baseRef is required: pass an explicit approved integration base (branch, tag, or commit hash).',
    )
  }

  const requestedBaseTip = await runGit(repoRoot, [
    'rev-parse',
    '--verify',
    `${baseRef}^{commit}`,
  ])
  if (!/^[0-9a-f]{40}$/.test(requestedBaseTip)) {
    throw new WorkerWorkspaceError(
      `base ref ${JSON.stringify(baseRef)} could not be resolved to a commit.`,
    )
  }

  const storyId = sanitizeBranchSegment(spec.storyId)
  const runId = deriveRunId(spec.workerId, spec.runId)
  const branchName = deriveBranchName(storyId, runId)
  const worktreesRoot = resolve(
    spec.worktreesRoot ?? join(dirname(repoRoot), DEFAULT_WORKTREES_DIRNAME),
  )
  const worktreePath = deriveWorktreePath(worktreesRoot, storyId, runId)
  assertExternalWorktreeRoot(repoRoot, worktreesRoot, worktreePath)

  const hasBranch = await branchExists(repoRoot, branchName)
  const hasPath = existsSync(worktreePath)

  if (!hasBranch && !hasPath) {
    return provisionWorkerWorkspace(spec)
  }
  if (!hasBranch && hasPath) {
    throw new WorkerWorkspaceError(
      `recovery found worktree path without expected branch ${branchName}: ${worktreePath}`,
    )
  }

  const registered = await listWorkerWorkspaces({ repoRoot })
  const branchWorktree = registered.find((item) => item.branchName === branchName) ?? null

  if (branchWorktree) {
    if (resolve(branchWorktree.worktreePath) !== resolve(worktreePath)) {
      throw new WorkerWorkspaceError(
        `recovery branch ${branchName} is registered at ${branchWorktree.worktreePath}, expected ${worktreePath}. Refusing to move or steal another workspace.`,
      )
    }
    if (!hasPath) {
      throw new WorkerWorkspaceError(
        `recovery branch ${branchName} is registered at ${worktreePath}, but the path is missing.`,
      )
    }
  } else {
    // The worktree may have been cleanly detached while the worker branch was
    // intentionally preserved. Reattach the existing branch; never create a
    // replacement branch and never reset its HEAD.
    if (hasPath) {
      throw new WorkerWorkspaceError(
        `recovery path ${worktreePath} exists but is not a registered git worktree for ${branchName}.`,
      )
    }
    mkdirSync(worktreesRoot, { recursive: true })
    await runGit(repoRoot, ['worktree', 'add', worktreePath, branchName])
  }

  // The original base is the common ancestor between the preserved worker
  // branch and the currently approved integration ref. This intentionally
  // keeps a recovered attempt on its original base even when integration has
  // advanced while the process was down; recovery is not an implicit rebase.
  const baseCommit = await runGit(repoRoot, [
    'merge-base',
    branchName,
    requestedBaseTip,
  ])
  if (!/^[0-9a-f]{40}$/.test(baseCommit)) {
    throw new WorkerWorkspaceError(
      `could not determine preserved base commit for recovered branch ${branchName}.`,
    )
  }

  const sharedLinks = await ensureSharedLinks(
    repoRoot,
    worktreePath,
    spec.sharedLinks ?? DEFAULT_SHARED_LINKS,
  )

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
