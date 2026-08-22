// ---------------------------------------------------------------------------
// ENG-21 — Isolated Worker Worktree Execution: public surface.
//
// Execution infrastructure. Story Board command semantics stay runtime-neutral.
// ---------------------------------------------------------------------------

export {
  provisionWorkerWorkspace,
  listWorkerWorkspaces,
  removeWorkerWorkspace,
  resolveRepoRoot,
  deriveBranchName,
  deriveRunId,
  deriveWorktreePath,
  sanitizeBranchSegment,
  GIT_BRANCH_PREFIX,
  DEFAULT_WORKTREES_DIRNAME,
  DEFAULT_SHARED_LINKS,
} from './provisioner'
export type { RemoveWorkerWorkspaceOptions } from './provisioner'
export type {
  WorkerWorkspace,
  WorkerWorkspaceSpec,
  WorkerWorkspaceListItem,
} from './types'
export { WorkerWorkspaceError } from './types'
