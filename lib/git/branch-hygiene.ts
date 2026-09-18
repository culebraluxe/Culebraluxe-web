/**
 * WHICH LEFTOVER BRANCHES MAY BE DELETED, and why the answer cannot come from ancestry.
 *
 * MEASURED 2026-09-18 on this repository. `scripts/sprint-cleanup.ts` judged a branch with
 * `git branch --merged main`, which asks whether the branch tip is an ANCESTOR of main. Work here
 * lands by rebase or cherry-pick, so a branch whose every patch is already in main is still not an
 * ancestor of it: `--merged` said "not merged" about 106 branches while `git cherry` said "already
 * applied" about their patches. A cleaner that cannot see landed work never cleans — 168 branches had
 * accumulated, and `git branch -d` refuses them for the same wrong reason, so deletion has to be a
 * FORCE delete whose proof is established first.
 *
 * The proof, in order of precedence:
 *   worktree     a worktree has it checked out (e.g. `cmd-01`) — never touched
 *   scaffolding  a test fixture by name: dogfood / smoke / dry-run / integration-chk
 *   landed       every commit already applied to main (`git cherry`: no `+` lines)
 *   unlanded     carries patches main has not seen — LEFT ALONE and named in the report
 *
 * And one rule that is about ownership rather than proof: only branches the TOOLING created are
 * deleted. `feat/*`, `v0/*`, `demo-lockdown/*` may be perfectly landed, but a person named them, and
 * a remote one can still be carrying a live preview deployment. Landed-and-human is reported, never
 * deleted.
 */

/** A branch left behind by a TEST RUN rather than by work: the Forge dogfoods itself. */
export const TEST_SCAFFOLDING_BRANCH = /(dogfood|smoke|dry-run|integration-chk|chk-)/

/** Branches this tooling created, and therefore may remove. */
export const MACHINE_OWNED_BRANCH = /^(agent\/|forge\/)|(dogfood|smoke|dry-run|integration-chk|chk-)/

export type BranchClass = 'worktree' | 'landed' | 'scaffolding' | 'unlanded'

export function branchClassOf(input: {
  name: string
  /** Commits on this branch whose patch main has NOT seen (`git cherry`: lines starting `+`). */
  unappliedPatches: number
  /** True when a worktree has this branch checked out — deleting it would yank a live workspace. */
  inWorktree: boolean
}): BranchClass {
  if (input.inWorktree) return 'worktree'
  if (TEST_SCAFFOLDING_BRANCH.test(input.name)) return 'scaffolding'
  return input.unappliedPatches === 0 ? 'landed' : 'unlanded'
}

/** Deletable only when the class proves it: a test fixture, or patches main already has. */
export function branchIsDeletable(cls: BranchClass): boolean {
  return cls === 'landed' || cls === 'scaffolding'
}

/** Both conditions for deletion, in one place so no caller can forget the ownership half. */
export function branchMayBeDeleted(name: string, unappliedPatches: number, inWorktree: boolean): boolean {
  return branchIsDeletable(branchClassOf({ name, unappliedPatches, inWorktree })) && MACHINE_OWNED_BRANCH.test(name)
}
