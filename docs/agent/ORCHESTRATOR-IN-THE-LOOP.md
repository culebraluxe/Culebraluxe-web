# Orchestrator in the loop — prototype

**Status:** prototype (read-only) · **Owner:** Captain / DevOps lane · **Date:** 2026-10-18

This is the first slice of "the orchestrator sits above Forge, Forge remains the deterministic
execution machine". It exists because the workflow engine is authoritative about *what runs next*,
while a dynamic orchestrator is better at *what matters now, what is blocked, and what needs Chris*.
The prototype adds a read surface and a decision loop; it adds no queue, no workflow node, and no
write path.

## What it reads

`scripts/orchestrate-status.sh` folds the live control plane into one decision packet through the
existing read-only verb `forge sql` (single SELECT, `begin read only`, no writes possible):

- `agent_work_item` — Ready / Claimed / Running work (the story execution queue);
- `forge_hold_record` — unresolved HOLDs (the canonical "needs a decision" path);
- `forge_decision` — active durable decisions;
- `forge_role_contract` — the latest role decisions in fields (not chat prose).

```sh
scripts/orchestrate-status.sh                 # human summary
scripts/orchestrate-status.sh --format json   # decision packet
scripts/orchestrate-status.sh --digest        # one-page daily digest
```

## Daily digest

`scripts/orchestrate-status.sh --digest` is the one-page read the orchestrator opens at the start of a
day (or after a long absence). It is the same read-only surface as the packet — no queue, no writes, no
state — but it answers the questions that change the next action instead of dumping every row:

- **STUCK** — `Claimed`/`Running` items the engine's own reaper would reap at its default window:
  `forge_reapable_claims(15)` (no heartbeat for 15 minutes — `FORGE_DEFAULT_STALE_MINUTES` — or a worker
  marked `stale` in `forge_worker_health`). The digest reuses that rule; it does not invent a second
  definition of "stale".
- **HOLDS** — unresolved `forge_hold_record` rows grouped by story **and reason**, with repeat counts
  (`3×`), the oldest age, and the resume target, so a repeated reason reads as one decision rather than
  twenty lines. Failure class is shown when one is recorded.
- **FAILED** — `agent_work_item.state = 'Error'` rows finished in the last 24 hours, with attempts and
  the first line of the error. Older failures are history, not today's action.
- **QUEUE / NEXT / ACTION** — ready/active counts, whether claims are paused in
  `forge_runtime_control`, the work-queue Pending/Running depth, the next Ready story, and the single
  next action (or an explicit "stay idle; do not invent work"). Active `forge_decision` rows close the
  page; candidate decisions awaiting promotion are listed when present.

Caps keep it one page (5 stuck lines, 10 HOLD groups, 5 failures, 5 active decisions); each capped
section still prints the total and how many rows it hid, so the digest never quietly drops state.

```sh
scripts/orchestrate-status.sh --digest
```

It is read-only in exactly the same way as the packet: it calls `forge sql`, which refuses non-SELECT
statements and runs inside `begin read only`. Resolving a HOLD, reclaiming a stuck claim, and re-queueing
a failed item happen on their canonical paths, never through this script.

## The loop

```text
read packet
  -> if Ready work: choose by priority/age and hand it to the EXISTING engine/worker
  -> if HOLDs: resolve through the canonical HOLD path, or present the decision to Chris
  -> if neither: say so; do not invent work
  -> record the decision canonically (forge_decision row, packet, MEMORY, HOLD resolution)
  -> verify evidence before claiming done
```

The orchestrator may choose **order and attention**. It may not claim a work item by hand, invent
a workflow transition, mark QA, publish, or write implementation code. Dispatch stays with the
forge engine/worker; delivery stays with the seven role services; release stays with DEV_OPS.

## First live read (2026-10-18)

```text
FORGE ORCHESTRATOR STATUS (target=prod, read-only)
  WORK     ready 0 · active 0
  NEXT     none ready — the queue is empty or everything is blocked
  HOLDS    20 open
  DECISIONS 7 active
  DECISION REQUIRED  true
```

That is the useful finding, not a failure: the machine has no Ready work because 20 HOLDs are
waiting on decisions (mostly `ENG-FORGE-C1-BUILD-INFO-01` QA failures and
`TST-INT-APPLE-001` missing candidate). An orchestrator that only drained `agent_work_item` would
report "idle"; this one reports the real bottleneck.

## Next

1. Add a bounded `--propose` mode that writes a decision *candidate* to the packet/decision path
   (never a HOLD resolution on someone else's authority).
2. Wire the Phase 2 advisory profiles as the "should we / why not" reasoning step behind resolution
   of these HOLDs (`forge/src/engine/advisory_profiles.rs` is the canonical catalogue).
3. Feed the packet into the existing Workflow `hold` resolution path, so a decision made above
   Forge still lands as a durable row rather than chat prose.

### Harness Evolution visibility

The daily digest now includes a `HARNESS` line: one line per domain with an active harness revision
(or an explicit "no active harness revision"). The read-only operator command is:

    cargo run -q -p cli -- forge harness-evolution status|revisions|experiments|experience [--format json]

It reads the migration-294 tables (`forge_harness_revision`, `forge_harness_activation`, ...). It does
not adopt, roll back, or probe; those are subsystem operations behind policy/adoption gates.
