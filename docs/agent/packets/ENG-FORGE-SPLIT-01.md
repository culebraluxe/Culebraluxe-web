# ENG-FORGE-SPLIT-01 — make the SPLIT lane real (and safe)

## Why

`lead_pre` can already choose `SPLIT`: `FORGE_SDLC-v1.xml` forks `split_dispatch`
(`minimum=2 maximum=8`, `plan-variable=splitPlan`, `join=split_join`) and the executor
runs siblings concurrently (`forge-executor.ts:279`, `cap = min(splitConcurrency, siblings)`).
Every child already gets its **own** 0-based slice (`engine.ts:1966` — `splitBranch:
planVariable[branchIndex]`) and its **own** workspace (`resolveForgeExecutionRunId(…, splitChild)`
→ `-split-<index>`; `"0"` is truthy so even the first child is isolated).

The lane is dark for three reasons only:

1. **The child is never handed its validated assignment.** The runner feeds it
   `formData.splitBranch` as prose; nothing ties it to the accepted proposal.
2. **Nothing verifies the join.** `split-join.ts` `reduceSplit` exists and is documented
   as "the deterministic reducer that feeds LEAD POST" — it has **no caller**. `split_join`
   completes on engine token counting alone, so `lead_post` can integrate with a missing,
   failed or conflicting child.
3. **The door is locked for a reason:** the historical incident (mangled branch, candidate
   captured from the wrong workspace) has no enforced provenance check.

## Scope

1. Resolve each child's assignment from the **re-validated accepted proposal**, using the
   engine's **0-based** `splitBranchIndex`, cross-checked against the engine-supplied
   `formData.splitBranch`; disagreement is a HOLD, never a silent pick.
2. **Fail closed**: a `smith_split_work` node that cannot resolve its assignment HOLDS —
   it must never fall through to `buildSmithWorkDecompositionDirective()` ("invent your
   own scope").
3. **Join gate**: before `lead_post` integrates, `reduceSplit` over the children's durable
   outcomes must be `joinSatisfied` (no missing / failed / cancelled / conflicting), or
   `lead_post` HOLDS naming the offenders.
4. **Provenance**: each child records its own candidate SHA on its own work item row, and
   the join requires one per expected child.
5. Flip `splitEnabled` / `maxSmiths` / `splitConcurrency` **in DEV only** and dogfood a real
   2-way split before any production posture changes.

## Not in scope

- Changing the engine's fork/join token semantics (ENG-FORGE owns that; if `split_join`
  and `reduceSplit` ever disagree about "all children done", that is a design decision
  escalated to the captain, not silently resolved).
- Enabling SPLIT on the production posture.

## Acceptance

- [ ] A split child with no resolvable assignment HOLDS (test).
- [ ] A split child's assignment comes from the accepted proposal, 0-based, and a mismatch
      with the engine slice HOLDS (test).
- [ ] `lead_post` under a SPLIT decision HOLDs when `reduceSplit` is not satisfied, naming
      missing/failed/conflicting children (test).
- [ ] Each child's candidate SHA is recorded per child and required at the join (test).
- [ ] `tsc --noEmit` clean; `pnpm test:forge:engine`, `pnpm test:app` green.
- [ ] DEV dogfood: one real 2-way split story reaches `split_join` → `lead_post` with two
      distinct child candidate SHAs and a satisfied reduction.

## Test mode

SCOPED — `pnpm exec tsx --test workflow_app/tests/forge-lead-routing.test.ts`,
`workflow_app/tests/forge-lead-routing-split.test.ts`, `pnpm exec tsc --noEmit`.

## Assay commands

- `pnpm exec tsx --test workflow_app/tests/forge-lead-routing-split.test.ts`
- `pnpm exec tsx --test workflow_app/tests/forge-lead-routing.test.ts`
- `pnpm exec tsc --noEmit`
