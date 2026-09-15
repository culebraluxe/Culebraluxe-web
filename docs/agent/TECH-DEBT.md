# TECH DEBT

What we know is not right, recorded so it is not lost and not re-discovered. Three rules:

1. **Blocking debt is at the top and gets fixed before ship.** Everything below it waits its turn.
2. **Debt has a name, a place and an exit.** "Later" is not an exit; "delete this line when X exists" is.
3. **Baselines are recorded debt.** The harness-lint baseline (`docs/agent/harness-lint-baseline.json`)
   holds findings we chose not to fix on day one; they are listed here in prose so a person reading
   this file knows they exist without running the gate.

Last reviewed: 2026-09-15 (PIRATE-01, ENG-FORGE-FACTORY-01 Phase 1, and Phase 2).

## Blocking

Nothing. `pnpm forge:harness`, `pnpm db:parity`, `pnpm test:app` and `pnpm smoke:prod` are all green as
of the review date; the Phase 1 probe passes on DEV with net zero, and `pnpm forge:decision check` reports
the seven decision rows and their mirrors agreeing.

## Cleanup with the next phase (not owed by anyone today)

- **Inspector's stale flag does not yet file work.** `decisionWritePolicy('inspector', 'flag-stale')` is
  written and tested, but the write it should trigger — a `learn` work item — belongs to Phase 3's learn
  loop. Exit: land it with Phase 3 and delete this line.

## Non-blocking, in the order I would pay them

1. **Per-card KIND in the cockpit sorter.** Phase 1 shows the kind mix on the batch line
   (`summarizeKinds`, from `forge_batch_item.kind`) but not a chip per card. Exit: add `kind` to the
   sorter-card shape in `lib/sorter-board.ts`, pass it through `buildSorterCards`, render it in the
   ENGINE BATCH column, and cover it in `workflow_app/tests/sorter-board.test.ts`. Half a day, cosmetic.
2. **`agent-runtime/write-policy.test.ts` does not exist**, and `docs/agent/packets/FORGE-GATES-01.md:83`
   lists it in that packet's Assay commands, so that packet's allow-list names a file that was never
   written. Exit: either write the test or delete the reference. Found by packet-lint rule 10; baselined.
3. **Two stale citations in old packets**, both baselined: `docs/agent/packets/ENG-FORGE-SPLIT-01.md`
   cites `engine.ts` and `PROJECTS-WORKSPACE-07.md` cites `db/document-form-instance.ts`; neither file
   exists anywhere. `PROJECTS-WORKSPACE-12.md` also cites `components/portal/projects-workspace.tsx:889-908`
   while the file is 877 lines. Exit: correct the citation or delete the line.
4. **Five pre-existing `ENG-FORGE-V9` topology failures** in `pnpm test:agent-runtime` (231 tests, 226
   pass). They pre-date every story in this session and are unrelated to app code. Exit: either the
   topology XML version and the test's expectation get reconciled, or the five tests are re-pointed.
5. **`.next` accumulates duplicate generated files** (`cache-life.d 10.ts` …) on release builds, which
   breaks `tsc`/`next build` until pruned by hand. Exit: fix the source in the prebuilt build path
   (`scripts/vercel-build-prod.sh`) rather than pruning. Not urgent; it costs a minute when it bites.
6. **`app/api/build-info/route 2.ts`** is an untracked editor-save duplicate sitting inside a route
   folder. Exit: delete it. Left alone because deleting another writer's file is not mine to do.
7. **Eight skill packs name no repo path that exists** (`pnpm forge:packet-lint` rule 11 warns:
   cruiser, forms, knip, neon, semgrep, serena, ui, workflow). Exit: anchor each pack to the code it
   describes, or delete the pack. The warning exists so the count is visible.
8. **`pnpm forge:harness` is not wired into any hook or CI.** Nothing runs it automatically: it is a
   command a person or a lane runs. Deliberate for now (the repo has no hooks; adding them is a
   decision, not a detail). Exit: decide, then wire it or keep it manual on purpose in writing.
9. **Production lags `main`** by design between releases (`pnpm smoke:prod` reports the gap in commits).
   Not debt unless it goes unshipped for something that matters. Exit: `bash scripts/vercel-build-prod.sh`
   then `bash scripts/vercel-deploy-prod.sh`, which ends in the live smoke.

## Deliberately not debt (do not "fix" these)

- **No third model policy, no seventh kind.** `lib/forge-kind.ts` is closed on purpose; the packet's
  stop condition says a third policy is a HOLD, not a feature.
- **`MEMORY.md` still holds the long-form incident narrative.** Phase 2 introduces `forge_decision` as
  the store that outlives an agent; until then MEMORY.md is not a duplicate, it is the only copy.
- **The engine runs PROD only.** DEV is where the Phase 1 probe runs because a probe is not a lane.
