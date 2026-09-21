# start-ruling-end

- status: active
- domain: forge
- source: captain
- owner: captain
- evidence: d764825a
- promoted: 2026-09-16
- supersedes: 

Every Forge lane records only START, RULING, and END; all other facts are metadata that may clarify but never overwrite the ruling.

*The captain, 2026-09-16: "you get Start, RULING, END — anything more you have to justify very very
very strongly on why it needs to be there."*

This note records what that rule deleted, what it cost to learn, and where each part is pinned so it
cannot quietly come back.

## The rule

A lane does three things and the machine records three facts:

1. **START** — the lane began: story, node, lane, when. One writer: the engine at claim.
2. **RULING** — the one answer. For a measuring lane it is **PASS or FAIL**; for a producing lane it is
   **delivered or not delivered**. One writer: the lane's own end.
3. **END** — the run closes with the ruling copied in, and `ended_at`. Derived, never re-decided.

Everything else — attempts, cost, notes, static gate, smoke, deliverable re-asks — is **metadata about
the ruling**. It may add detail. It may never overwrite the answer.

## What it deleted, and what each cost

Every one of these was a middle step that decided something the ruling had already decided, and every
one was found by *running* the machine, not by reading it.

| Deleted | What it did | What it cost |
|---|---|---|
| The QA sha conjunct (`normalizeAgentFinishForRole`) | Finalized an Assay run **Hold** unless the evidence proved `verifiedSha === candidateSha` | QA records no git identity by rule, so the conjunction could never be true: **every** QA run was recorded Hold while every frozen command passed. Measured on run `5a1494f6` — verdict PASS, summary "Assay PASS \| … -> exit 0", run row Hold |
| The remote scope base (`workspaces?.baseRef ?? 'origin/main'`) | Diffed a candidate against `origin/main` | Under NO TREES `workspaces` is undefined, and under deferred publishing origin lags a whole sprint: measured 17 commits behind, 31 paths reported "outside" a one-file assignment, on work whose proof passed 9/9 |
| Candidate equality with HEAD | Re-affirmed a lane's candidate only if it still equalled HEAD | Any commit landing after the story's own — another lane's, or the engine's own fix — pushed HEAD past it and the retry held on `smith-candidate` with the work committed. Measured on `da4003cb` |
| Stale failure markers | Left `failure_class` / `failed_release_stage` on the row after the stage succeeded | `devops_resume_router` routes **on** `failedReleaseStage`, so a resolved failure could send a story back to the stage that already succeeded |

## The corollaries

- **One writer per fact.** If two records can disagree about one thing, one of them is a bug. Tonight the
  same QA run produced four records that disagreed two ways.
- **The record derives from the ruling.** A run's status is a copy of the verdict, written after the
  verdict exists — never decided separately, and never decided earlier.
- **A field nothing writes is a lie.** `architect_brief_updated_at` was null on every engine-written
  brief; `base_commit_hash` is null on every run row. A column that only one of its writers maintains
  describes a machine that does not exist.
- **NO TREES has a second half.** Removing worktrees is not done while code still *assumes* a worktree
  exists. Every failure tonight traced back to a middle step that quietly fell back to a remote ref or a
  worktree path because the tree was gone.

## Where it is pinned

- `agent-runtime/candidate-assay-handoff.test.ts` — a clean verdict finalizes clean; an unclean verdict
  still holds and still says why.
- `legacy/workflow_app/tests/story-scope-base.test.ts` — the scope base is the story's own, never a guess.
- `legacy/workflow_app/tests/storyboard.test.ts` — the architect's writer stamps when the brief last changed.
- `legacy/workflow_app/tests/forge-release-markers.test.ts` — a resolved release clears the failure it resolved.

Commit trail: `703d3d63` (verdict is the ruling), `c25a9041` (story base), `d1085c63` (ancestry),
`2936c5e8` (brief timestamp), `6cbaeb9d` (empty plan fails, no run means no verdict).
