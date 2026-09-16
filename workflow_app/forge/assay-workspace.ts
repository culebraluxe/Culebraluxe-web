// Assay measures the candidate worktree or it does not measure at all.
//
// The pin (agent-runtime-role-runner) detaches a lane worktree at the candidate SHA.
// b33e938 stopped that detach from running against the operator checkout. It did not
// stop Assay from *executing proofs there* when no worktree was provisioned
// (`roleCwd === process.cwd()`). That is the 2026-09-15 loop with the safety catch
// off: a verdict from the wrong tree that looks like evidence.
//
// So: QA nodes with no worktree do not measure. The error is a verification gap
// for a human, never a code defect for repair.

const ASSAY_NODES = new Set(['qa_verify', 'fast_qa_verify'])

export const ASSAY_NO_WORKTREE =
  'ASSAY_WORKSPACE_NOT_CANDIDATE: assay has no worktree (cwd is the operator checkout); ' +
  'refusing to measure a tree that is not the candidate (a gap for a human, not a defect to repair)'

export function isAssayNode(nodeId: string): boolean {
  return ASSAY_NODES.has(nodeId)
}

/** Null when the node may measure. Message when it must not. */
export function assayWorkspaceRefusal(input: {
  nodeId: string
  roleCwd: string
  operatorCwd: string
}): string | null {
  if (!isAssayNode(input.nodeId)) return null
  if (input.roleCwd === input.operatorCwd) return ASSAY_NO_WORKTREE
  return null
}
