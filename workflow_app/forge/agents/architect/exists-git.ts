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
