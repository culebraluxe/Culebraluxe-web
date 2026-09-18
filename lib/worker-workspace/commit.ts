import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

import { fileOf, within } from '../../workflow_app/forge/agents/shared/path'

const execFileAsync = promisify(execFile)

export type WorkerCommitResult = {
  commitHash: string | null
  changed: boolean
  /** Paths the commit would have swept that the lane did not declare. Non-empty means
   *  either the commit was REFUSED before it was made (commitHash null) or the commit
   *  exists and its sha is returned WITH these paths named. A commit that exists on the
   *  branch is never reported as nothing. */
  refused?: string[]
}

export type WorkerCommitOptions = {
  /** The write surface this lane declared. When present, only paths inside it may be
   *  committed: any other dirty path in the checkout refuses the commit by name.
   *  Absent preserves the legacy commit-everything behaviour. */
  allowedScope?: string[]
}

/**
 * ENG-FORGE-SURFACE-SUPPLIER-01 — the machine marker a lane stamps into its own
 * instructions so the harness-owned commit seam can carry the SAME declared surface
 * it was launched with. The surface is written once by the runner (the one place that
 * resolves the accepted assignment) and read once here; it is never re-derived.
 */
export const ALLOWED_SCOPE_MARKER = 'FORGE_ALLOWED_SCOPE:'

/** Render the lane's declared surface as a single machine line, or '' when none. */
export function renderAllowedScopeMarker(surface: readonly string[]): string {
  const paths = surface.map((entry) => entry.trim()).filter(Boolean)
  if (paths.length === 0) return ''
  return `${ALLOWED_SCOPE_MARKER} ${JSON.stringify(paths)}`
}

/** Read back the declared surface the runner stamped, or null when absent/unparseable. */
export function parseAllowedScopeMarker(text: string | null | undefined): string[] | null {
  if (!text) return null
  const at = text.lastIndexOf(ALLOWED_SCOPE_MARKER)
  if (at < 0) return null
  const rest = text.slice(at + ALLOWED_SCOPE_MARKER.length)
  const lineEnd = rest.indexOf('\n')
  const line = (lineEnd >= 0 ? rest.slice(0, lineEnd) : rest).trim()
  try {
    const parsed: unknown = JSON.parse(line)
    if (!Array.isArray(parsed)) return null
    const paths = parsed
      .filter((entry): entry is string => typeof entry === 'string' && entry.trim().length > 0)
      .map((entry) => entry.trim())
    return paths.length > 0 ? paths : null
  } catch {
    return null
  }
}

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

/**
 * `git status --porcelain` output is POSITIONAL: each line is `XY <path>` and the
 * two status columns (and the space after them) are data, not decoration. This read
 * is deliberately NEVER trimmed — trimming the whole string or each line drops the
 * first line's leading status space, and `slice(3)` then eats the first character of
 * its path (`agent-runtime/...` became `gent-runtime/...`), so an in-scope modified
 * tracked file was refused as out of scope.
 */
async function gitPorcelainStatus(cwd: string): Promise<string> {
  const { stdout } = await execFileAsync('git', ['status', '--porcelain'], {
    cwd,
    encoding: 'utf8',
  })
  return stdout.replace(/\r?\n+$/, '')
}

/** One path per porcelain line, rename-aware (`R  old -> new` names `new`). */
function porcelainPaths(status: string): string[] {
  return status
    .split('\n')
    .filter((line) => line.trim().length > 0)
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
  const status = await gitPorcelainStatus(worktreePath)
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
    // Backstop: the commit itself must carry only declared paths. A commit now exists
    // on the branch, so it is NEVER reported as nothing: the sha is returned WITH the
    // refusal, and the caller can see both facts instead of a null that hides the commit.
    const committed = await git(worktreePath, ['show', '--name-only', '--format=', 'HEAD'])
    const extras = committed
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean)
      .filter((path) => !isDeclared(path, declared))
    const commitHash = await git(worktreePath, ['rev-parse', 'HEAD'])
    if (extras.length > 0) {
      return { commitHash: commitHash || null, changed: true, refused: extras }
    }
    return { commitHash: commitHash || null, changed: true }
  }

  await git(worktreePath, ['add', '-A'])
  await git(worktreePath, ['commit', '-m', message])
  const commitHash = await git(worktreePath, ['rev-parse', 'HEAD'])
  return { commitHash: commitHash || null, changed: true }
}
