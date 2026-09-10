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
