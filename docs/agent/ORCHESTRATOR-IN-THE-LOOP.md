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
```

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
