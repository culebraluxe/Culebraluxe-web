# ENG-FORGE-DOCTOR-01 — forge:doctor: read the control plane without changing it

## Goal

One command answers "is the control plane clear?" before any test, instead of hand-writing the query every
time. `pnpm forge:doctor` prints instances, open tasks, open work items, active engine claims and the age
of the oldest active claim, reports whether the scheduled WORKER is alive from its own log, and renders the
mailbox POSTCARD block — and it writes nothing, ever.

## Why

Requested after a night of clearing stale claims by hand before every run. `pnpm forge:clean` is the
writer; this is its read-only sibling, so an operator can look before deciding to clean. **Read-only is a
hard requirement: a doctor that mutates is not a doctor.**

The liveness half is why it earns the name. The scheduled worker died at its own preflight on every tick
since 2026-09-03 — 463 invocations, exit 2, never reaching `pnpm agent:work` — because macOS TCC denies a
launchd-spawned process access to `~/Documents`, and nothing in the cockpit or the control plane said so.
The board looked healthy and empty. A doctor that reports a clear control plane while the worker has been
dead for twelve days is not answering the operator question.

The postcard half was added 2026-09-15 when the mailbox protocol went live and the reply had to be
assembled by hand from five commands; the doctor already reads every one of those facts, so printing them
is the same read, not a second tool.

> Written 2026-09-15 while closing the ladder rung: the story row carried this contract from the start, but
> no packet existed on disk, so the Lead held at `lead_pre` for an unroutable file — a story cannot be
> routed, scoped, or proven from a database row alone. This file is that contract, in the repo, where the
> roles and the lint can read it.

## Scope

- `scripts/forge-doctor.ts` (new): the read-only operator command; the CLI half and the reads.
- `workflow_app/forge/forge-doctor-report.ts` (new): the pure formatter — control-plane report AND the
  mailbox postcard block, both rendered from values passed in, so both are unit-testable with no database.
- `package.json`: register the `pnpm forge:doctor` script.

No other existing file changes.

## Do not touch

- Any writer path: `forge-clean`, claim/release, `storyboard_story_run` writes, task state.
- `db/agent-work.ts` claim/insert/update functions and `db/storyboard.ts` write paths.
- The Grok↔DeepSeek mailbox protocol beyond *rendering* the postcard facts.

## Architect brief

**Seam contract for this story (read before declaring scope).** `scripts/forge-doctor.ts` and
`workflow_app/forge/forge-doctor-report.ts` **do not exist yet** — this story creates them. A declared seam
must exist on the pinned baseRef as a blob **or a tree** (`workflow_app/forge/agents/architect/assess.ts:53-61`),
and the Lead routes a new file by its **directory** seam. So declare the directory, not a sibling file:

- for `scripts/forge-doctor.ts`, declare the seam **`scripts/`** (the tree) — **not** `scripts/forge-batch-status.ts`,
  which is a different file and does not cover the new one;
- for `workflow_app/forge/forge-doctor-report.ts`, declare the seam **`workflow_app/forge/`**;
- for the test, the seam **`workflow_app/tests/`**.

A sibling file is not a seam for a new file: it names a surface the new file is not on, and the routing
validator then refuses the whole route. Held on 2026-09-15 for exactly this — the Lead named the missing
`scripts/` seam and confirmed the route is otherwise one Smith (five SAME_UNIT findings, no split).


- `node --import tsx --test workflow_app/tests/forge-doctor-report.test.ts`

Test mode: **SCOPED**. No FULL regression for this story.

## Acceptance criteria

1. `pnpm forge:doctor` prints instances, open tasks, open work items and active engine claims, plus the age
   of the oldest active claim.
2. It writes nothing: no update, no insert, no claim. Read-only is the contract, not a preference.
3. It reports whether the scheduled worker is alive from its own logs — the newest invocation and the most
   recent failure reason — read from `AGENT_WORKER_LOG_DIR` (default `~/Library/Logs/CulebraLuxe`).
4. It renders the POSTCARD block: board-vs-table agreement, active decision count, the 7-day ROI rows and
   the newest learn-pass attempt.
5. Every fact it prints is read from data that already exists on the run base ref.
6. The rendering is a pure function with unit tests, including the empty-control-plane case, the
   board-drifted-from-table case and the worker-failing case.
7. A missing or empty worker log is never reported as healthy — absence of evidence is not health.

## Out of scope (stop if you start these)

- A second control-plane tool, dashboard, or scheduled job.
- Fixing the TCC/launchd worker death itself (that is an operator action, recorded here as the reason the
  liveness half exists).
- Any schema change, any migration, any new table or view.

## Sign-off

- Builder reports the exact files changed and the Assay command above.
- Reviewer checks: does it write anything (it must not); is the formatter pure; is a missing log reported
  honestly rather than as healthy.
