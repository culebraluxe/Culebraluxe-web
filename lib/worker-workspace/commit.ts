import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)

export type WorkerCommitResult = {
  commitHash: string | null
  changed: boolean
}

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

/**
 * Commit all non-ignored worker changes from the OUTER Forge process.
 *
 * The model sandbox only needs workspace file access. Git metadata for an
 * isolated worktree lives under the primary repo's .git/worktrees directory,
 * so commit ownership belongs to Forge itself, not to the model process.
 *
 * Returns null when the worker produced no changes. Never pushes or merges.
 */
export async function commitWorkerWorkspaceChanges(
  worktreePath: string,
  message: string,
): Promise<WorkerCommitResult> {
  const status = await git(worktreePath, ['status', '--porcelain'])
  if (!status) return { commitHash: null, changed: false }

  await git(worktreePath, ['add', '-A'])
  await git(worktreePath, ['commit', '-m', message])
  const commitHash = await git(worktreePath, ['rev-parse', 'HEAD'])
  return { commitHash: commitHash || null, changed: true }
}
