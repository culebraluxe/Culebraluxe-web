# ENG-FORGE-SYNC-01 — Ship-time board sync

**Status:** implemented (tool + tests + writer seam). Board apply pending captain's go.
**Priority:** Critical · ENGINEERING / TECH / SCOPED
**Filed:** 2026-09-11, at the captain's request.

## Goal

Make the PROD Story Board always reflect shipped work: the moment a story ships,
its completion and its run/work-product numbers land in PROD without a human
reconciling anything.

## Why (the motivating incident)

PROJECTS-WORKSPACE-01..12 shipped as merged commits on `main`, but the PROD board
still read **Planned at 20-75%** — because no `storyboard_story_run` and no
`agent_work_item` rows existed for that series in PROD. Nothing connected shipped
code to the board, so twelve stories had to be reconciled **by hand**.

The board was not merely stale. It silently disagreed with git, and nothing told
the operator that the evidence lived elsewhere.

## Scope (four parts)

1. **Run provenance** — a run's `execution_environment` is read and surfaced, so
   a board entry cannot imply evidence it cannot point at.
2. **Ship-time sync** — derive completion from durable evidence (commits naming
   the story id on the release branch + runs/work items where present).
3. **Environment guard** — Forge executes against PROD; a non-PROD target fails
   closed, and a run that happened elsewhere is *visible*, never silent.
4. **No invented numbers** — where evidence is absent, say so. Never estimate,
   and never re-run work to manufacture a number.

## Implementation

| Piece | Where |
| --- | --- |
| Derivation (pure, no DB/git/fs) | `workflow_app/forge/forge-board-sync.ts` |
| Tests (17) | `workflow_app/tests/forge-board-sync.test.ts` |
| Adapter (git + Neon + CLI) | `scripts/forge-board-sync.ts` → `pnpm forge:board-sync` |
| Writer seam | `db/forge-story-state.ts` → `markForgeStoryShippedComplete` |

`deriveBoardSync` invariants, each executable as a test:

- already Complete → `no-change` (idempotent; safe to re-run)
- shipping commits, not Complete → `complete` 100%
- no shipping commits, no runs → `no-change` (`no-ship-evidence`) — stays Planned
- runs but no shipping commits → `no-change` (`run-evidence-only`)
- **docs/packet commit only → `no-change` (`packet-only`)**
- absent run evidence → recorded as ABSENT, never estimated
- a non-PROD run → `environmentWarning`, and the note carries a WARNING line

### Lesson found on live data: a packet commit is not shipped work

The first live dry run "completed" `PROJECTS-WORKSPACE-13` because a commit matched:
`docs(forge): story packet for PROJECTS-WORKSPACE-13`. Writing a story is not
building it. `classifyShipCommits` now separates **shipping** commits from
**docs-only** commits; packet-only matches produce `packet-only` and write nothing.
A `complete` decision that also saw docs commits says what it excluded.

## Verification

- `pnpm test:forge:engine` → 369 tests, **368 pass, 0 fail**, 4 skipped.
- Live dry run (read-only) over the open board: **73 open stories → 1 reconciles to
  Complete, 72 unchanged**, 3 reported as SHIPPED BUT HELD, and 11 stories reported
  as carrying DEV run evidence (never silent).
- `pnpm exec tsc --noEmit` — clean for the touched files.

### Two more false positives the live scan caught (both now rules + tests)

1. **A bare-id packet commit.** `ENG-FORGE-V5-11: lead dev qa topology packet` has
   no conventional type, so a `docs(...)`-only filter missed it. `isShippingCommit`
   now also rejects any commit whose subject mentions `packet`.
2. **A prefix collision.** `OPS-11` matched inside `OPS-11A`, so OPS-11A's commit
   would have completed a *different* story. `storyIdMatcher` now requires the id
   to be a whole token.

Effect on the live list: the naive matcher would have written **9** completions.
After both rules it writes **1** — the other 8 were packets, prefix collisions, or
human holds. This is the story working as intended: it refuses to invent.

### A human Hold is not repealed by shipped code

`Hold` and `Deferred` are deliberate human states. A shipped commit does not
repeal them, so the sync reports `held-shipped` and writes nothing — the operator
sees the state and releases the hold on purpose. (On 2026-09-11 that surfaced
`ENG-FORGE-SHAPE-01`, `ENG-FORGE-V5-08`, `ENG-FORGE-WORKSPACE-01`.)

## Known limits (stated, not hidden)

- **Engine-start wiring is not done.** The fail-closed guard
  (`assertForgeExecutionTarget`) exists, is tested, and is called by the sync path,
  but the run lane does not yet refuse a non-PROD target at launch. That is the
  remaining half of part 3 and the next slice.
- **Board UI surface.** Provenance is read, reported by the tool, and written into
  the note; it is not yet rendered as a column in the portal board.
- **Docs-only heuristic.** Only `docs(...)` commits and subjects mentioning
  `packet` are excluded. A `chore(...)` version bump that happens to name a story
  would still count as shipping evidence.
