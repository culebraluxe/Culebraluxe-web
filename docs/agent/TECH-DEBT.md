# TECH DEBT

What we know is not right, recorded so it is not lost and not re-discovered. Three rules:

1. **Blocking debt is at the top and gets fixed before ship.** Everything below it waits its turn.
2. **Debt has a name, a place and an exit.** "Later" is not an exit; "delete this line when X exists" is.
3. **Baselines are recorded debt.** The harness-lint baseline (`docs/agent/harness-lint-baseline.json`)
   holds findings we chose not to fix on day one. It is **empty** as of 2026-09-15, the same day it was
   recorded — both of its debts were paid rather than carried, and the file says so in prose.

Last reviewed: 2026-09-15 (the debt-clearing pass: release-path bundler, the V9 live throw, `.next`
duplicates, the empty lint baseline, KIND chips, and the harness wired into the release build).

## Blocking

Nothing. `pnpm forge:harness` (48 harness tests, packet-lint 0 failures / 1 warning / 0 baselined),
`pnpm db:parity`, `pnpm test:app` and `pnpm smoke:prod` are all green as of the review date; `tsc` is clean
and `pnpm test:agent-runtime` is 237 pass / 0 fail; the Phase 1 probe passes on DEV with net zero,
`pnpm forge:decision check` reports the seven decision rows and their mirrors agreeing, `probe-learn-dedupe`
proves the learn loop's de-dupe against a real Postgres, and `pnpm forge:roi` reads the live rollup.

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
  `legacy/db/database-gateway.ts` now import bare `fs`/`crypto` for that reason. Exit: if Next stops bundling
  instrumentation for Edge, revert to the `node:` form and delete this line.

## Paid on 2026-09-15 (kept so the register visibly moves)

- **The dual build path is gone: the release runs exactly what QA runs.** `next build --webpack` is now
  `pnpm run build`, so `scripts/vercel-build-prod.sh` → `vercel build --prod` builds with the same bundler
  and the same `next.config.mjs` edge fallbacks the QA command exercises. `turbopack: {}` stays declared
  because the dev server is Turbopack; the webpack edge fallbacks are what make bare `fs`/`crypto` stubbable
  in the Edge compilation. Proven by running the release build after the change (harness gate green inside
  it, artifact produced, nothing deployed).
- **The five `ENG-FORGE-V9` topology failures were not five stale tests — one of them was a live throw.**
  `agent-runtime/forge-topology.ts` retyped the expected FORGE_SDLC version as a literal `1` while the loader
  (`legacy/workflow_app/definitions/forge-sdlc.ts`) had long since exported `FORGE_SDLC_VERSION = 6` and read
  `FORGE_SDLC-v6.xml`. The guard therefore failed closed on a path that `scripts/forge-orchestrate-wake.ts`
  calls (`runForgeHydrate`, `runForgeFollow`), and that script is imported by `scripts/agent-work.ts` — the
  live worker. The throw was gated behind the night plan, so it would have killed **the first unattended
  night run**, which is the same run the learn loop and the ROI backfill are waiting on. Fixed at the source
  of truth: the guard and the test both import `FORGE_SDLC_VERSION` (one constant, no second literal), the
  stale `-v1.xml` header and error messages now name what is actually loaded, and `pnpm test:agent-runtime`
  is 237 pass / 0 fail.
- **The release artifact is never built from a duplicated `.next` tree.** `scripts/vercel-build-prod.sh` now
  clears `.next` before `vercel build` (measured: 2,035 stray files that morning, 0 immediately after a
  clean release build). The upstream duplication is **not** solved — it is an open item below — but it can no
  longer reach a release artifact.
- **The harness now gates the release.** `pnpm forge:harness` runs inside `scripts/vercel-build-prod.sh`
  before the artifact is built, so a drifted manifest, a hand-edited vendor block or a packet citing a dead
  path stops a release. It proved itself on its first run: writing the missing test made the FORGE-GATES-01
  manifest stale and the build aborted rather than shipping the stale artifact.
- **The lint baseline is EMPTY.** Both recorded debts were paid instead of carried: the four V4 packets'
  free-text `## Skills` sections now name `workflow` (the pack that applies), and the three stale citations
  were re-pointed at the files that hold the code today. `pnpm forge:packet-lint`: 0 failure(s), 1 warning(s),
  **0 baselined** — down from 22 warnings with 8 baselined.
- **`agent-runtime/write-policy.test.ts` exists** (7 tests) — the file `docs/agent/packets/FORGE-GATES-01.md`
  listed in its Assay commands, covering the commit boundary and the rewind that makes a non-builder commit
  unreachable from the worktree.
- **Per-card KIND chips** (`lib/sorter-board.ts`, `app/portal/tech/page.tsx`,
  `components/portal/tech/story-kanban-board.tsx`, `legacy/workflow_app/tests/sorter-board.test.ts`): a staged card
  now shows its own kind, and an unread kind renders no chip rather than a default. The page reads
  `listStagingBatchItems()` once and uses it for both the batch roster and the cards.
- **Seven skill packs are anchored** to real paths (forms → `lib/forms/form-instance-io.ts`, neon →
  `legacy/db/database-gateway.ts`, ui → `app/globals.css`, workflow → `legacy/workflow_app/forge/agent-runtime-role-runner.ts`,
  knip → `./knip.json`, cruiser → `./.dependency-cruiser.js`, semgrep → `scripts/forge-packet-lint.ts`), and
  cruiser / knip / ripwire / rtk / semgrep / serena are in `KNOWN_SKILLS`, so packets can actually load them.
- **Stray file deleted:** `app/api/build-info/route 2.ts` (an untracked editor-save duplicate, byte-identical
  to `route.ts`, verified before removing it).
- **Production is current** (`ce553e9`), so the "lags `main` by design" line is gone; it was a fact to
  report, not debt to carry.

## Open, in the order I would pay them

1. **One skill pack has no anchor and says so.** `docs/agent/skills/serena.md` is the single remaining
   `skill-not-anchored` warning (`pnpm forge:packet-lint`: 0 failure(s), 1 warning(s), 0 baselined). Serena
   is a global tool with no config, index or adapter committed here, so there is nothing truthful to point
   it at. Exit: when the runtime adapter exposes serena as a tool, point the pack at the adapter; otherwise
   delete the pack. The pack states this in its own body rather than carrying a fake citation.
2. **Inspector's stale flag still files no work.** `decisionWritePolicy('inspector', 'flag-stale')` is
   written and tested, and Phase 3 has the machinery that could open the item, but nothing connects the two:
   `pnpm forge:decision` has no `flag` command. Exit: add one that opens a `kind=learn` item carrying the
   decision key as its pattern key, then delete this line.
3. **A burst of findings files one item and defers the rest, by design.** The learn loop's window advances
   on every successful pass, so the other nine findings in a noisy commit are reported as `deferred` and are
   not filed by a later pass (the window has moved past them). That is the packet's cap working, but it means
   a big commit's second finding needs a human to notice the `deferred` line. Exit: if this bites, keep an
   unfiled-findings queue (a table, not a log line) and let the loop drain one per pass.
4. **The learn loop has never run unattended on PROD.** It is wired into `scripts/agent-work-entry.ts` after
   the batch fire and is proven on DEV (dry run + the de-dupe probe), but no launchd pass has executed it
   against the live board yet. The first night run is the end-to-end proof; watch `app_error` for
   `forge-learn-pass-failed` and the worker log for the `learn:` lines.
5. **Kind + policy has never been printed by a live claim**, for the same reason: no work item has been
   dispatched since Phase 1. The copy and the log line are unit-tested and the columns exist in both
   environments; the first claim is what makes it observed rather than asserted.
6. **Every ROI row reads `unrecorded` until a post-Phase-1 attempt finishes.** `pnpm forge:roi` on PROD shows
   506 attempts in one bucket because kind/policy were added on 2026-09-15 and no attempt has run since.
   Exit: it resolves itself on the first night run; if it still reads `unrecorded` after one, the copy at
   dispatch is not happening and that is a Phase 1 bug, not a rollup bug. Phase 4's own acceptance
   (a cheap fix and a judgment feature as two rows) is not demonstrated for the same reason.
7. **Something upstream re-creates duplicate files inside `.next`, and they break `tsc`.** The honest
   current state, with the measurements that produced it: **2,035** stray files (`cache-life.d 3.ts` and
   friends) the morning of 2026-09-15; **0** immediately after a release build that started from an empty
   `.next`; **377 back after the next build**, carrying **preserved mtimes** (04:10 and 05:31 on files a
   fresh 11:0x build had just created), which points at a cache restore rather than an in-place copy. `tsc`
   then fails with `TS6200`/`TS2300` duplicate identifiers from `.next/types/*.d N.ts`, and `rm -rf .next`
   clears it (`tsc` clean, verified). Ruled out: no test or script in this repo runs a build, and nothing
   outside `.next` holds a copy. Exit: reproduce with `vercel build --force` (or by clearing the local build
   cache) and name the cache directory that feeds it; until then the release path is clean by construction
   and the local remedy is `rm -rf .next`.

## Deliberately not debt (do not "fix" these)

- **No third model policy, no seventh kind.** `lib/forge-kind.ts` is closed on purpose; the packet's
  stop condition says a third policy is a HOLD, not a feature.
- **`MEMORY.md` still holds the long-form incident narrative.** Phase 2 introduces `forge_decision` as
  the store that outlives an agent; until then MEMORY.md is not a duplicate, it is the only copy.
- **The engine runs PROD only.** DEV is where the Phase 1 probe runs because a probe is not a lane.
- **`cost_usd` is still empty almost everywhere** (vendor-reported actuals arrive late): the ROI strip
  reports widget coverage instead of pretending to know dollars. True before Phase 4 and recorded so nobody
  "fixes" it by inventing a rate.
- **The ROI window is attempts-by-work-item, not spend-by-run.** One story with three attempts counts three
  times, which is the right answer to "what did the night batch cost" and the wrong one to "what did this
  story cost". Exit: a per-story view, if anyone ever asks for one — until then it is a scope choice, not a
  defect.
- **Two modules import bare `fs`/`crypto` instead of the `node:` scheme** (`lib/execution-target.ts`,
  `legacy/db/database-gateway.ts`). The `node:` form is not stubbable in the Edge compilation, so the bare form is
  load-bearing. Exit: if Next stops bundling instrumentation for Edge, revert to `node:` and delete this.
