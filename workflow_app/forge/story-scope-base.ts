// ---------------------------------------------------------------------------
// THE STORY'S OWN BASE COMMIT — what a scope check must diff against.
//
// The scope gate asked `git` to diff a candidate against `origin/main` (the fallback when no worktree base
// exists, which under NO TREES is always). That was fine while every lane published its own commit; under a
// batch/sprint release, publish is DEFERRED, so origin lags by a whole sprint and the "candidate diff" is
// every commit made since the sprint began. Measured 2026-09-16 on ENG-FORGE-QA-CONSISTENCY-01: local main was
// 17 commits ahead of origin, and the gate reported 31 files "outside a1" — every file touched that night —
// and held work that was complete with its proof passing.
//
// The story's base is not a remote ref. It is where THE STORY started: the parent of the first commit the
// story itself made. That is derivable from records the machine already keeps (each run's commit hash), so
// nothing new has to be written to know it.
//
// Pure by construction: the git read is injected, so the rule is testable without a repository.
// ---------------------------------------------------------------------------

export type StoryCommitHash = string | null | undefined

/** The parent of the earliest commit this story made, or null when the story has made none yet. */
export function storyScopeBase(
  storyCommits: ReadonlyArray<StoryCommitHash>,
  readParent: (commit: string) => string | null,
): string | null {
  // Runs arrive newest-first from every reader in this repo (`storyboard_story_run` order), but the rule must
  // not depend on that: the base is defined by the EARLIEST commit, so find it explicitly.
  let earliest: string | null = null
  for (const candidate of storyCommits) {
    const sha = normalize(candidate)
    if (sha) earliest = sha // the last one seen is the earliest, given newest-first
  }
  if (!earliest) return null
  return normalize(readParent(earliest))
}

/**
 * THE RECORDED BASE WINS OVER A DERIVED ONE (ENG-FORGE-START-BASE-01).
 *
 * Each run records the HEAD it started from (`base_commit_hash`). The story's base is the
 * EARLIEST recorded value — the first lane's start — never a later lane's post-commit HEAD.
 * Values arrive newest-first; a null/blank entry (a lane that could not read its base)
 * contributes nothing, and no recorded value at all returns null so the caller derives.
 */
export function recordedScopeBase(recorded: ReadonlyArray<StoryCommitHash>): string | null {
  let earliest: string | null = null
  for (const candidate of recorded) {
    const sha = normalize(candidate)
    if (sha) earliest = sha // the last valid seen is the earliest, given newest-first
  }
  return earliest
}

function normalize(value: StoryCommitHash): string | null {
  const sha = (value ?? '').trim().toLowerCase()
  return /^[0-9a-f]{40}$/.test(sha) ? sha : null
}
