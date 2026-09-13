import { execFileSync } from 'node:child_process'

/** Sync existence against a pinned SHA. Directories and blobs both count. */
export function existsOnGitBaseRef(repoDir: string): (baseRef: string, repoPath: string) => boolean {
  return (baseRef, repoPath) => {
    try {
      execFileSync('git', ['-C', repoDir, 'cat-file', '-e', `${baseRef}:${repoPath}`], {
        stdio: 'pipe',
      })
      return true
    } catch {
      return false
    }
  }
}
