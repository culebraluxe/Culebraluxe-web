# ENG-FORGE-SPLIT-DOGFOOD-01 — prove the SPLIT lane end to end (DEV)

## Why

`ENG-FORGE-SPLIT-01` wired the lane: each child resolves its OWN accepted assignment
(0-based engine index, cross-checked against the engine slice, fail-closed), records its
own candidate SHA, and `lead_post` refuses to integrate unless the join is satisfied.
None of that has ever executed, because no story has produced a real fan-out.

This story is deliberately shaped as **two independent units with disjoint files and
separate proof commands**, so an honest Lead proposes SPLIT and the whole path runs:
`lead_pre --SPLIT--> split_dispatch --> 2 × smith_split_work --> split_join --> lead_post`.

## Units

- **A** — `smithContractFromAssignment` (`workflow_app/forge/forge-split-handoff.ts`)
  must set `prohibitedScope` to the **sibling** assignments' surfaces, so isolation is
  enforced by the machine contract, not only by rendered prose.
  Proof: `pnpm exec tsx --test workflow_app/tests/forge-lead-routing-split.test.ts`
- **B** — `splitJoinHoldReasons` (`workflow_app/forge/split-join.ts`) must report
  **duplicated** terminal child outcomes as a named reason.
  Proof: `pnpm exec tsx --test workflow_app/tests/forge-split-join.test.ts`

## Acceptance

- [ ] `prohibitedScope` carries sibling surfaces; a test asserts it.
- [ ] Duplicated terminal outcomes produce a named join-hold reason; a test asserts it.
- [ ] Two children reach a terminal state, each with its OWN recorded candidate SHA.
- [ ] `lead_post` integrates only with a satisfied join.
- [ ] Existing lead-routing / split-join tests unchanged and green.

## Test mode

SCOPED — the two assay commands above only.

## Assay commands

- `pnpm exec tsx --test workflow_app/tests/forge-lead-routing-split.test.ts`
- `pnpm exec tsx --test workflow_app/tests/forge-split-join.test.ts`

## Run

```sh
FORGE_SPLIT_ENABLED=true FORGE_SPLIT_MAX_SMITHS=2 FORGE_SPLIT_CONCURRENCY=2 \
  node --env-file=.env.local --import tsx scripts/forge-engine-worker.ts \
  --story ENG-FORGE-SPLIT-DOGFOOD-01 --work-type FEATURE
```

## Run log

### 2026-09-10 — dogfood attempts (the lane is wired; still dark by default)

Each attempt fixed a real blocker, in this order:

1. Lead SPLIT accepted, but the child could not be tied to an assignment → **durable
   accepted proposal** (migration 147) instead of re-validating mutable context.
2. Assignment resolved, but the second child could not be claimed → **parallel-group
   enqueue** (a story-wide "existing row" lookup collapsed siblings onto one row).
3. Claimed, but the claim failed on `agent_work_item_one_parallel_slot` → **one slot
   convention** (1-based) across the enqueue and the child-assignment writer.
4. Claim failed under the *system-wide* single-active rule → **claim scope** now
   honours parallel groups (only an active SERIAL item blocks a split child).
5. Both children claimed and ran, then the second was refused as workspace theft:
   the branch name truncated the run id to 40 chars and dropped `-split-N` → **branch
   naming fixed** (this is the original "mangled branch" incident).
6. Both children then executed in their own worktrees; with `splitConcurrency=2` a
   bare driver error surfaced during simultaneous progress writes → **open**.

Observed live: the fork creates 2 children with the engine's 0-based index and each
child's own slice (`idx 0 → 'a'`, `idx 1 → 'b'`), both are claimable concurrently, and
each provisions a distinct branch/worktree. **Not yet observed:** both children
finishing, a satisfied join, and `lead_post` integrating.

Note: one attempt had the Lead choose HOLD (model variance) and the engine correctly
parked at the human gate — a legitimate outcome, not a lane failure.
