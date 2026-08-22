// ---------------------------------------------------------------------------
// ENG-21 — Isolated Worker Worktree Execution: types.
//
// Execution infrastructure only. Story Board command semantics stay
// runtime-neutral — this module knows git, never business commands.
// ---------------------------------------------------------------------------

/** Everything the provisioner needs to create ONE isolated worker context. */
export type WorkerWorkspaceSpec = {
  /** Canonical story id (branch segment source, sanitized). */
  storyId: string
  /** Worker identity that owns the workspace (recorded in the result). */
  workerId: string
  /**
   * EXPLICIT approved integration base ref (branch name, tag, or commit hash).
   * Never derived from whatever mutable HEAD happens to be current — the
   * mechanism resolves it to a fixed commit at creation time and pins the
   * workspace there.
   */
  baseRef: string
  /** Optional explicit run id; a unique short id is derived when absent. The
   *  worker command passes the durable work-item id so the branch is
   *  deterministic per work item. */
  runId?: string
  /** Primary checkout containing the shared .git. Derived via git when absent. */
  repoRoot?: string
  /** Directory where worker worktrees live (must be OUTSIDE the primary
   *  checkout). Defaults to `../Culebraluxe-worktrees` next to the repo. */
  worktreesRoot?: string
  /** Repo-rooted resources symlinked into the workspace (shared-safe, never
   *  copied; gitignored in the primary checkout). */
  sharedLinks?: string[]
}

/** One provisioned isolated worker context. */
export type WorkerWorkspace = {
  storyId: string
  workerId: string
  runId: string
  /** `agent/<story>/<run>` — unique local branch owned by this worker. */
  branchName: string
  /** Absolute path of the isolated worktree (outside the primary checkout). */
  worktreePath: string
  /** The base ref that was requested (as supplied). */
  baseRef: string
  /** The commit the workspace was created from (baseRef resolved). */
  baseCommit: string
  /** Shared resources symlinked into the workspace (relative to repo root). */
  sharedLinks: string[]
}

/** Read-only projection for `status` (parsed from `git worktree list`). */
export type WorkerWorkspaceListItem = {
  branchName: string
  worktreePath: string
  head: string
  storyId: string | null
  runId: string | null
}

/** A deterministic, actionable provisioning/cleanup failure. */
export class WorkerWorkspaceError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'WorkerWorkspaceError'
  }
}
