/**
 * THE MACHINE CLEANS UP AFTER ITSELF, AT THE MOMENT ITS WORK LANDS.
 *
 * WHY THIS EXISTS (ENG-FORGE-LANE-TEARDOWN-01, 2026-09-18): `provisionWorkerWorkspace` creates a branch
 * per lane attempt and `removeWorkerWorkspace` deliberately PRESERVES it ("the branch survives"), which
 * is right at teardown time — an abandoned lane's commits are cheaper than lost code — but nothing ever
 * collected them afterwards. Measured that night: **168 branches** had accumulated, 65 of them fixtures
 * from dogfood and smoke rehearsals, and the counter-wipe could not see any of them because it judged
 * landing by ANCESTRY while work here lands by rebase and cherry-pick.
 *
 * The rule is the one already proven in lib/git/branch-hygiene.ts, applied at the one moment when the
 * answer is certain: a candidate has just been published, so every patch of its lane branch is in main
 * BY CONSTRUCTION. The proof is still re-derived here rather than assumed — `git cherry` against the
 * published base — so a lane whose work did NOT land (a refused candidate, a competing lane still in
 * flight) is left alone, and a branch a person named is never touched.
 *
 * Cleanup can never break a release: the caller treats a failure here as a note, not an error.
 */
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

import { MACHINE_OWNED_BRANCH, branchIsDeletable, branchClassOf } from '../git/branch-hygiene'

const execFileAsync = promisify(execFile)

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

export type LaneSweepResult = {
  /** Machine-owned branches whose every patch is in the base branch, now deleted. */
  deleted: string[]
  /** Branches examined and deliberately kept (unlanded work, a worktree, or a person's name). */
  kept: number
}

/** Commits on `branch` whose patch `base` has not seen (`git cherry`: lines starting `+`). */
async function unappliedPatchCount(repoRoot: string, base: string, branch: string): Promise<number> {
  const out = await git(repoRoot, ['cherry', base, branch])
  return out.split('\n').filter((line) => line.startsWith('+')).length
}

export async function sweepLandedLaneBranches(input: {
  repoRoot: string
  /** The branch that just received the work. Default `main`. */
  baseRef?: string
}): Promise<LaneSweepResult> {
  const base = (input.baseRef ?? 'main').trim() || 'main'
  const repoRoot = input.repoRoot

  const heads = await git(repoRoot, ['for-each-ref', '--format=%(refname:short)', 'refs/heads'])
  const branches = heads
    .split('\n')
    .map((b) => b.trim())
    .filter((b) => b.length > 0 && b !== base)

  // A branch a worktree has checked out is a LIVE workspace: deleting it would yank the directory out
  // from under whoever is using it (the same rule the counter-wipe applies to `cmd-01`).
  const inWorktree = new Set(
    (await git(repoRoot, ['worktree', 'list', '--porcelain']))
      .split('\n')
      .filter((line) => line.startsWith('branch refs/heads/'))
      .map((line) => line.replace('branch refs/heads/', '').trim()),
  )

  const deleted: string[] = []
  let kept = 0
  for (const branch of branches) {
    if (!MACHINE_OWNED_BRANCH.test(branch)) {
      kept++
      continue
    }
    const unapplied = await unappliedPatchCount(repoRoot, base, branch)
    const cls = branchClassOf({
      name: branch,
      unappliedPatches: unapplied,
      inWorktree: inWorktree.has(branch),
    })
    if (!branchIsDeletable(cls)) {
      kept++
      continue
    }
    await git(repoRoot, ['branch', '-D', branch])
    deleted.push(branch)
  }
  return { deleted, kept }
}
