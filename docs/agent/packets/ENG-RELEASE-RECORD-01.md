# ENG-RELEASE-RECORD-01 — make the release record queryable, so "what is in production" is a query

## Goal

Turn the local append-only release record written by `pnpm release` (`scripts/release-record.sh`) into a
queryable record the cockpit and the Forge chain can read: one row per release attempt with the commit SHA,
branch, tree state, build result, deploy result and outcome, so "what is actually in production" is answered
by a query instead of by a terminal that has since been closed.

## Why

With two scripts the evidence of a release lived only in the terminal that ran them. On 2026-09-15 the only
honest local answer to "is this live?" was "my commits are on main", and nothing about whether anything had
been built or deployed — the release facts existed nowhere durable. The bash process closes that for a human
reading a file; this story closes it for everything else, including the chain.

Two deliberate non-goals, because they are the wrong instinct and the whole point of the request: the Forge
chain must **never build and never deploy**. A build and a deploy on every story is ceremony that buys
nothing. DEV_OPS references a receipt; it does not perform a release. And no new release mechanism is
introduced — the master process stays the only way a release happens.

## Scope

- `legacy/db/migrations/<next>_release_record.sql` (new): the `release_record` table — sha, branch, tree_state,
  build_result, deploy_result, outcome, started_at, ended_at, recorded_by. No defaults that invent facts.
- `db/release-record.ts` (new): the writer/reader boundary, normalizing driver values before they leave it
  (timestamps to ISO strings, counts to JS numbers) per the repository-boundary rule in `AGENTS.md`.
- `scripts/release-record.sh`: write through `db/release-record.ts` instead of appending markdown, keeping
  the markdown as a readable mirror rather than the source of truth.
- The TECH portal page that already shows Forge facts: surface the last N release records.

## Do not touch

- `scripts/vercel-build-prod.sh` and `scripts/vercel-deploy-prod.sh` — they stay exactly as they are, and
  stay usable on their own for build-only or deploy-only work.
- Any per-story chain step that would perform a build or a deploy.

## Assay (SCOPED)

- `node --import tsx --test legacy/workflow_app/tests/release-record.test.ts` (new) — the writer's normalization and
  the reader's ordering, including a failed release recorded and never dropped.

Test mode: **SCOPED**. No FULL regression for this story.

## Acceptance criteria

1. A release attempt writes exactly one row, whether it succeeded or failed; a failed release is recorded,
   never skipped, because a missing row is not evidence of a clean release.
2. The row carries the facts a human needed on 2026-09-15: SHA, branch, tree state, build result, deploy
   result, outcome, start and end.
3. Nothing invents a fact: a step that did not run records `skipped`, not a success.
4. The chain can ask "is there a receipt for this SHA?" and get an answer without building anything.
5. The bash process remains the only way a release happens; nothing else gains the ability to deploy.

## Out of scope (stop if you start these)

- A second release mechanism, a queue, a scheduler, or an automatic release on merge.
- Deploying anything from inside the Forge chain.
- Rebuilding the Vercel scripts, or moving away from the local build the captain deliberately chose.

## Sign-off

- Builder reports the exact files changed and the Assay command above.
- Reviewer checks: does any chain step build or deploy (it must not); can a failed release be recorded; is
  any recorded fact invented rather than measured.
