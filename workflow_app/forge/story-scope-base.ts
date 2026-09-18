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

/**
 * THE LANE'S OWN CHANGES, NOT THE INTERVAL (ENG-FORGE-SCOPE-OWN-CHANGES-01).
 *
 * A scope check used to diff a candidate against the STORY's base, so every commit
 * that landed between that base and the candidate — the operator's, another lane's —
 * was attributed to the lane. Measured 2026-09-18: a SPLIT child was held for nine
 * files, every one of them the operator's, committed while the child sat in flight.
 *
 * A lane is responsible only for what IT wrote. Its own changes are the union of each
 * lane-authored commit's diff against its PARENT (`git diff --name-only <c>^ <c>`), so
 * a foreign commit in the range is excluded even when it touches the assignment
 * surfaces. The candidate is always a lane commit.
 *
 * The descendant check runs FIRST and fails closed: `git merge-base` would otherwise
 * silently pick an unrelated base for a candidate that does not descend from the
 * recorded base, and the diff would describe someone else's history. Pure by
 * construction — the git reads are injected, so the rule is testable without a repo.
 */
export type CandidateOwnChanges =
  | { ok: true; changedFiles: string[] }
  | { ok: false; reason: string }

export function candidateOwnChangedFiles(input: {
  candidateSha: StoryCommitHash
  recordedBase: StoryCommitHash
  laneCommits?: ReadonlyArray<StoryCommitHash>
  readChangedFiles: (commit: string) => ReadonlyArray<string>
  isAncestor: (ancestor: string, descendant: string) => boolean
}): CandidateOwnChanges {
  const candidate = normalize(input.candidateSha)
  const base = (input.recordedBase ?? '').trim()
  if (!candidate) {
    return { ok: false, reason: `candidate ${String(input.candidateSha ?? '(none)')} is not a commit` }
  }
  if (!base) {
    return { ok: false, reason: `candidate ${candidate} has no recorded base to measure against` }
  }
  if (!input.isAncestor(base, candidate)) {
    return {
      ok: false,
      reason: `candidate ${candidate} is not a descendant of its recorded base ${base}`,
    }
  }
  const commits = new Set<string>([candidate])
  for (const raw of input.laneCommits ?? []) {
    const sha = normalize(raw)
    if (sha) commits.add(sha)
  }
  const changed = new Set<string>()
  for (const sha of commits) {
    for (const path of input.readChangedFiles(sha)) {
      const clean = path.trim()
      if (clean) changed.add(clean)
    }
  }
  return { ok: true, changedFiles: [...changed].sort() }
}

function normalize(value: StoryCommitHash): string | null {
  const sha = (value ?? '').trim().toLowerCase()
  return /^[0-9a-f]{40}$/.test(sha) ? sha : null
}
