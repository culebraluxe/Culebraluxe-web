import { execFileSync } from 'node:child_process'

export type WritePolicy = {
  allowCommit: boolean
  allowDevDbWrite: boolean
}

export function writeBoundaryLines(policy: WritePolicy): string[] {
  if (policy.allowCommit) {
    return [
      'Work in the current repository. Verify your work by running tests/typecheck/build within the runtime policy above.',
      'Create a local git commit with the intended changes when the story requires it.',
      'Do NOT push. Do NOT mutate production data or schema. Report what you did.',
    ]
  }
  return [
    'POLICY: this role may NOT create a git commit and may NOT write DEV or PROD schema/data.',
    'Read the repo, run the allowed tests, and report evidence only.',
    'If you edited files while investigating, leave them unstaged and do not commit.',
    'Do NOT push.',
  ]
}

/** If a non-builder committed, drop the hash and rewind the worktree to base. */
export function revokeForbiddenCommit(input: {
  allowCommit: boolean
  workspace: string
  commitHash: string | null
  baseCommit?: string | null
}): { commitHash: string | null; violation: string | null } {
  if (input.allowCommit || !input.commitHash) {
    return { commitHash: input.commitHash, violation: null }
  }
  if (input.baseCommit) {
    try {
      execFileSync('git', ['reset', '--hard', input.baseCommit], {
        cwd: input.workspace,
        encoding: 'utf8',
      })
    } catch {
      // evidence still records the violation even if rewind fails
    }
  }
  return {
    commitHash: null,
    violation: `WRITE POLICY VIOLATION: non-builder created commit ${input.commitHash}; hash dropped and worktree rewind requested.`,
  }
}
