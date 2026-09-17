import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

import { fileOf, within } from '../../workflow_app/forge/agents/shared/path'

const execFileAsync = promisify(execFile)

export type WorkerCommitResult = {
  commitHash: string | null
  changed: boolean
  /** Paths the commit would have swept that the lane did not declare. Non-empty means
   *  the commit was REFUSED (commitHash null) and these paths are named to the caller. */
  refused?: string[]
}

export type WorkerCommitOptions = {
  /** The write surface this lane declared. When present, only paths inside it may be
   *  committed: any other dirty path in the checkout refuses the commit by name.
   *  Absent preserves the legacy commit-everything behaviour. */
  allowedScope?: string[]
}

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

/** One path per porcelain line, rename-aware (`R  old -> new` names `new`). */
function porcelainPaths(status: string): string[] {
  return status
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => line.slice(3).trim())
    .map((rest) => {
      const arrow = rest.lastIndexOf(' -> ')
      return arrow >= 0 ? rest.slice(arrow + 4) : rest
    })
    .map((p) => p.replace(/^"|"$/g, ''))
    .filter(Boolean)
}

function isDeclared(path: string, surface: readonly string[]): boolean {
  const file = fileOf(path)
  if (!file) return false
  return surface.some((entry) => {
    const area = fileOf(entry)
    return area ? within(file, area) : false
  })
}

/**
 * Commit worker changes from the OUTER Forge process.
 *
 * The model sandbox only needs workspace file access. Git metadata for an isolated
 * worktree lives under the primary repo's .git/worktrees directory, so commit ownership
 * belongs to Forge itself, not to the model process.
 *
 * When the lane declares a surface, the checkout is shared: any dirty path OUTSIDE that
 * surface belongs to another writer, so the commit is REFUSED and the extra paths are
 * returned rather than swept (`git add -A` used to do exactly that sweep). With no
 * declared surface the legacy behaviour is unchanged.
 *
 * Returns null when the worker produced no changes. Never pushes or merges.
 */
export async function commitWorkerWorkspaceChanges(
  worktreePath: string,
  message: string,
  options: WorkerCommitOptions = {},
): Promise<WorkerCommitResult> {
  const status = await git(worktreePath, ['status', '--porcelain'])
  if (!status) return { commitHash: null, changed: false }

  const declared = [
    ...new Set(
      (options.allowedScope ?? [])
        .map((entry) => fileOf(entry))
        .filter((entry): entry is string => Boolean(entry)),
    ),
  ]
  if (declared.length > 0) {
    const overWide = porcelainPaths(status).filter((path) => !isDeclared(path, declared))
    if (overWide.length > 0) {
      return { commitHash: null, changed: false, refused: overWide }
    }
    await git(worktreePath, ['add', '--', ...declared])
    const staged = await git(worktreePath, ['diff', '--cached', '--name-only'])
    if (!staged) return { commitHash: null, changed: false }
    await git(worktreePath, ['commit', '-m', message])
    // Backstop: the commit itself must carry only declared paths.
    const committed = await git(worktreePath, ['show', '--name-only', '--format=', 'HEAD'])
    const extras = committed
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean)
      .filter((path) => !isDeclared(path, declared))
    if (extras.length > 0) return { commitHash: null, changed: false, refused: extras }
    const commitHash = await git(worktreePath, ['rev-parse', 'HEAD'])
    return { commitHash: commitHash || null, changed: true }
  }

  await git(worktreePath, ['add', '-A'])
  await git(worktreePath, ['commit', '-m', message])
  const commitHash = await git(worktreePath, ['rev-parse', 'HEAD'])
  return { commitHash: commitHash || null, changed: true }
}
