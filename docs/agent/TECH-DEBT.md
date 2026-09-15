# TECH DEBT

What we know is not right, recorded so it is not lost and not re-discovered. Three rules:

1. **Blocking debt is at the top and gets fixed before ship.** Everything below it waits its turn.
2. **Debt has a name, a place and an exit.** "Later" is not an exit; "delete this line when X exists" is.
3. **Baselines are recorded debt.** The harness-lint baseline (`docs/agent/harness-lint-baseline.json`)
   holds findings we chose not to fix on day one; they are listed here in prose so a person reading
   this file knows they exist without running the gate.

Last reviewed: 2026-09-15 (PIRATE-01, ENG-FORGE-FACTORY-01 Phases 1, 2, 3 and 4, and the release of `ce553e9`).

## Blocking

Nothing. `pnpm forge:harness`, `pnpm db:parity`, `pnpm test:app` and `pnpm smoke:prod` are all green as
of the review date; the Phase 1 probe passes on DEV with net zero, `pnpm forge:decision check` reports the
seven decision rows and their mirrors agreeing, `probe-learn-dedupe` proves the learn loop's de-dupe
against a real Postgres, and `pnpm forge:roi` reads the live rollup.

## Released (2026-09-15)

- **Production is current as of `ce553e9`.** `bash scripts/vercel-build-prod.sh` → `bash scripts/vercel-deploy-prod.sh`
  shipped the harness gates, the board repair, and Factory Phases 1–4; the deploy verified the live sha and
  ended in the live smoke (4/4, including the sha assertion), and protected routes answer 307 to `/login`
  rather than 500, which is what proves the edge middleware and the edge-safe instrumentation load.
- **Two build paths now have to keep working, and that is a new obligation, not a detail:**

  1. `vercel build --prod` → `pnpm run build` → `next build` (**Turbopack**, Next 16's default). It REFUSES a
     project that has a `webpack` config and no `turbopack` config — that error is what broke the first
     release attempt, and it only appeared once the edge fix added a `webpack` key. `turbopack: {}` in
     `next.config.mjs` is the fix, and it must stay.
  2. `pnpm exec next build --webpack` (the QA command in `AGENTS.md`) — this is the path that needs the
     edge-only builtin fallbacks, because webpack walks instrumentation's dynamic import graph.

  Exit: pick ONE bundler. Either make `pnpm run build` be `next build --webpack` so the release runs exactly
  what QA runs, or drop the webpack config and verify Turbopack passes the same edge graph. Until then, a
  green `next build --webpack` does NOT prove the release builds — measured today, webpack was green while
  `vercel build` failed.
- **`node:`-scheme imports are not stubbable in the edge compilation.** `lib/execution-target.ts` and
  `db/database-gateway.ts` now import bare `fs`/`crypto` for that reason. Exit: if Next stops bundling
  instrumentation for Edge, revert to the `node:` form and delete this line.

## Cleanup with the next phase (not owed by anyone today)

- **Inspector's stale flag still does not file work.** `decisionWritePolicy('inspector', 'flag-stale')` is
  written and tested, and Phase 3 now has the machinery that could open the item, but nothing connects the
  two: `pnpm forge:decision` has no `flag` command. Exit: add one that opens a `kind=learn` item carrying
  the decision key as its pattern key, then delete this line.
- **A burst of findings files one item and defers the rest, by design.** The learn loop's window advances on
  every successful pass, so the other nine findings in a noisy commit are reported as `deferred` and are not
  filed by a later pass (the window has moved past them). That is the packet's cap working, but it means a
  big commit's second finding needs a human to notice the `deferred` line. Exit: if this bites, keep an
  unfiled-findings queue (a table, not a log line) and let the loop drain one per pass.
- **The learn loop has never run unattended on PROD.** It is wired into `scripts/agent-work-entry.ts` after
  the batch fire and is proven on DEV (dry run + the de-dupe probe), but no launchd pass has executed it
  against the live board yet. The first night run is the end-to-end proof; watch `app_error` for
  `forge-learn-pass-failed` and the worker log for the `learn:` lines.
- **Every ROI row reads `unrecorded` until a post-Phase-1 attempt finishes.** `pnpm forge:roi` on PROD shows
  506 attempts in one bucket because kind/policy were added today and no attempt has run since. Exit: it
  resolves itself on the first night run; if it still reads `unrecorded` after one, the copy at dispatch is
  not happening and that is a Phase 1 bug, not a rollup bug.
- **The ROI window is attempts-by-work-item, not spend-by-run.** One story with three attempts counts three
  times, which is correct for "what did the night batch cost" and wrong for "what did this story cost".
  Exit: a per-story view, if anyone ever asks for one.
- **`cost_usd` is still empty almost everywhere** (vendor-reported actuals arrive late): the strip reports
  widget coverage instead of pretending to know dollars. Already true before this story; recorded so nobody
  "fixes" it by inventing a rate.

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
