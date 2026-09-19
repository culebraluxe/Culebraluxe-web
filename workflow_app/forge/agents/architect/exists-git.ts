import { execFileSync } from 'node:child_process'
import { gitBinary } from '../../../../lib/worker-workspace/provisioner'

/** Sync existence against a pinned SHA. Directories and blobs both count. */
export function existsOnGitBaseRef(repoDir: string): (baseRef: string, repoPath: string) => boolean {
  return (baseRef, repoPath) => {
    try {
      execFileSync(gitBinary(), ['-C', repoDir, 'cat-file', '-e', `${baseRef}:${repoPath}`], {
        stdio: 'pipe',
      })
      return true
    } catch {
      return false
    }
  }
}

/**
 * IS THIS COMMIT ALREADY ON THE PINNED BASE? (FORGE-VERIFY-EXISTING-COMPLETE-01)
 *
 * The direct-to-QA route verifies a candidate that already exists, and the validator requires it to be ON
 * the base ref rather than merely present in the repository: verifying a commit main has never seen would
 * be verifying work that is not the repository's — a false pass with an honest-looking sha. Ancestry is the
 * right test (`merge-base --is-ancestor`), not existence.
 *
 * A git failure (unknown base, unknown sha, unreadable repository) answers FALSE: "we could not prove it is
 * on the base" and "it is not on the base" lead to the same refusal, and neither may be read as yes.
 */
export function commitOnGitBaseRef(repoDir: string): (baseRef: string, sha: string) => boolean {
  return (baseRef, sha) => {
    try {
      execFileSync(gitBinary(), ['-C', repoDir, 'merge-base', '--is-ancestor', sha, baseRef], {
        stdio: 'pipe',
      })
      return true
    } catch {
      return false
    }
  }
}
